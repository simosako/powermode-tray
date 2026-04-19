/// Power mode overlay GUIDs for Windows 11
/// Balanced:              00000000-0000-0000-0000-000000000000
/// Best Performance:      ded574b5-45a0-4f42-8737-46345c09c238
/// Best Power Efficiency: 961cc777-2547-4f9d-8174-7d86181b8a7a
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Condvar, Mutex, Once, OnceLock};
use std::time::Duration;

use windows_sys::core::GUID as WinGuid;
use windows_sys::Win32::Foundation::{HANDLE, HMODULE, WIN32_ERROR};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Power::{
    GetSystemPowerStatus, PowerSettingRegisterNotification, PowerSettingUnregisterNotification,
    DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY, POWERBROADCAST_SETTING, SYSTEM_POWER_STATUS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_CALLBACK;

/// GUID struct matching Windows GUID layout (128-bit)
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl core::fmt::Debug for GUID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7],
        )
    }
}

const GUID_BALANCED: GUID = GUID {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
};

const GUID_BEST_PERFORMANCE: GUID = GUID {
    data1: 0xded574b5,
    data2: 0x45a0,
    data3: 0x4f42,
    data4: [0x87, 0x37, 0x46, 0x34, 0x5c, 0x09, 0xc2, 0x38],
};

const GUID_BEST_POWER_EFFICIENCY: GUID = GUID {
    data1: 0x961cc777,
    data2: 0x2547,
    data3: 0x4f9d,
    data4: [0x81, 0x74, 0x7d, 0x86, 0x18, 0x1b, 0x8a, 0x7a],
};

const MENU_ID_BALANCED: u32 = 1001;
const MENU_ID_BEST_PERFORMANCE: u32 = 1002;
const MENU_ID_BEST_POWER_EFFICIENCY: u32 = 1003;

// Function pointer types for powrprof.dll APIs.
type FnPowerGetOverlay = unsafe extern "system" fn(*mut GUID) -> u32;
type FnPowerSetOverlay = unsafe extern "system" fn(*const GUID) -> u32;

// ── Cached DLL handle ──────────────────────────────────────────
// powrprof.dll is a system DLL that stays loaded for the process lifetime.
// We load it once and cache the HMODULE to avoid repeated LoadLibraryW
// calls that would otherwise leak reference counts.

use crate::util::to_wide;

static INIT_LIB: Once = Once::new();
static mut POWRPROF_LIB: HMODULE = std::ptr::null_mut();
static POWER_GET_EFFECTIVE_OVERLAY: OnceLock<Option<FnPowerGetOverlay>> = OnceLock::new();
static POWER_GET_ACTUAL_OVERLAY: OnceLock<Option<FnPowerGetOverlay>> = OnceLock::new();
static POWER_SET_ACTIVE_OVERLAY: OnceLock<Option<FnPowerSetOverlay>> = OnceLock::new();

const ENERGY_SAVER_STATUS_UNKNOWN: u32 = u32::MAX;
const ENERGY_SAVER_STATUS_OFF: u32 = 0;
const ENERGY_SAVER_STATE_QUERY_TIMEOUT: Duration = Duration::from_millis(250);

const GUID_ENERGY_SAVER_STATUS: WinGuid = WinGuid {
    data1: 0x550e8400,
    data2: 0xe29b,
    data3: 0x41d4,
    data4: [0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00, 0x00],
};

static ENERGY_SAVER_STATE: AtomicU32 = AtomicU32::new(ENERGY_SAVER_STATUS_UNKNOWN);
static ENERGY_SAVER_READY: Mutex<bool> = Mutex::new(false);
static ENERGY_SAVER_READY_CVAR: Condvar = Condvar::new();
static INIT_ENERGY_SAVER: Once = Once::new();
static mut ENERGY_SAVER_REGISTRATION: HPOWERNOTIFY = 0;

/// Get the cached HMODULE for powrprof.dll, loading it on first call.
unsafe fn get_powrprof_lib() -> HMODULE {
    INIT_LIB.call_once(|| {
        POWRPROF_LIB = LoadLibraryW(to_wide("powrprof.dll").as_ptr());
        if POWRPROF_LIB.is_null() {
            crate::debug_log!("Failed to load powrprof.dll");
        }
    });
    POWRPROF_LIB
}

/// Dynamically load a function from powrprof.dll by name.
/// The DLL handle is cached; only one LoadLibraryW call occurs per process.
unsafe fn load_powrprof_fn(name: &[u8]) -> Option<*const ()> {
    let lib = get_powrprof_lib();
    if lib.is_null() {
        return None;
    }
    let proc = GetProcAddress(lib, name.as_ptr());
    if proc.is_none() {
        crate::debug_log!(
            "Failed to find {:?} in powrprof.dll",
            core::str::from_utf8(&name[..name.len() - 1]).unwrap_or("?")
        );
        return None;
    }
    Some(proc.unwrap() as *const ())
}

fn get_power_get_effective_overlay() -> Option<FnPowerGetOverlay> {
    *POWER_GET_EFFECTIVE_OVERLAY.get_or_init(|| unsafe {
        load_powrprof_fn(b"PowerGetEffectiveOverlayScheme\0")
            .map(|fp| core::mem::transmute::<*const (), FnPowerGetOverlay>(fp))
    })
}

fn get_power_get_actual_overlay() -> Option<FnPowerGetOverlay> {
    *POWER_GET_ACTUAL_OVERLAY.get_or_init(|| unsafe {
        load_powrprof_fn(b"PowerGetActualOverlayScheme\0")
            .map(|fp| core::mem::transmute::<*const (), FnPowerGetOverlay>(fp))
    })
}

fn get_power_set_active_overlay() -> Option<FnPowerSetOverlay> {
    *POWER_SET_ACTIVE_OVERLAY.get_or_init(|| unsafe {
        load_powrprof_fn(b"PowerSetActiveOverlayScheme\0")
            .map(|fp| core::mem::transmute::<*const (), FnPowerSetOverlay>(fp))
    })
}

#[cfg(debug_assertions)]
unsafe fn call_overlay_getter(name: &str, func: FnPowerGetOverlay) -> Result<GUID, u32> {
    let mut guid = GUID_BALANCED;
    let ret = func(&mut guid);
    crate::debug_log!("{} => ret={}, guid={:?}", name, ret, guid);
    if ret == 0 {
        Ok(guid)
    } else {
        Err(ret)
    }
}

#[cfg(not(debug_assertions))]
unsafe fn call_overlay_getter(func: FnPowerGetOverlay) -> Result<GUID, u32> {
    let mut guid = GUID_BALANCED;
    let ret = func(&mut guid);
    if ret == 0 {
        Ok(guid)
    } else {
        Err(ret)
    }
}

#[cfg(debug_assertions)]
unsafe fn call_overlay_setter(name: &str, func: FnPowerSetOverlay, guid: &GUID) -> u32 {
    let ret = func(guid);
    crate::debug_log!("{} => ret={}", name, ret);
    ret
}

#[cfg(not(debug_assertions))]
unsafe fn call_overlay_setter(func: FnPowerSetOverlay, guid: &GUID) -> u32 {
    func(guid)
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMode {
    Balanced,
    BestPerformance,
    BestPowerEfficiency,
}

impl PowerMode {
    pub fn label(self) -> &'static str {
        match self {
            PowerMode::Balanced => "Balanced",
            PowerMode::BestPerformance => "Best Performance",
            PowerMode::BestPowerEfficiency => "Best Power Efficiency",
        }
    }

    pub fn to_stored_u32(self) -> u32 {
        self as u32
    }

    pub fn from_stored_u32(value: u32) -> Option<Self> {
        match value {
            x if x == PowerMode::Balanced as u32 => Some(PowerMode::Balanced),
            x if x == PowerMode::BestPerformance as u32 => Some(PowerMode::BestPerformance),
            x if x == PowerMode::BestPowerEfficiency as u32 => Some(PowerMode::BestPowerEfficiency),
            _ => None,
        }
    }

    pub fn to_menu_id(self) -> u32 {
        match self {
            PowerMode::Balanced => MENU_ID_BALANCED,
            PowerMode::BestPerformance => MENU_ID_BEST_PERFORMANCE,
            PowerMode::BestPowerEfficiency => MENU_ID_BEST_POWER_EFFICIENCY,
        }
    }

    pub fn from_menu_id(id: u32) -> Option<Self> {
        match id {
            MENU_ID_BALANCED => Some(PowerMode::Balanced),
            MENU_ID_BEST_PERFORMANCE => Some(PowerMode::BestPerformance),
            MENU_ID_BEST_POWER_EFFICIENCY => Some(PowerMode::BestPowerEfficiency),
            _ => None,
        }
    }

    fn to_guid(self) -> GUID {
        match self {
            PowerMode::Balanced => GUID_BALANCED,
            PowerMode::BestPerformance => GUID_BEST_PERFORMANCE,
            PowerMode::BestPowerEfficiency => GUID_BEST_POWER_EFFICIENCY,
        }
    }

    fn from_guid(guid: &GUID) -> PowerMode {
        if *guid == GUID_BEST_PERFORMANCE {
            PowerMode::BestPerformance
        } else if *guid == GUID_BEST_POWER_EFFICIENCY {
            PowerMode::BestPowerEfficiency
        } else {
            PowerMode::Balanced
        }
    }
}

fn guid_eq(left: &WinGuid, right: &WinGuid) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

unsafe extern "system" fn energy_saver_callback(
    _context: *const c_void,
    _notification_type: u32,
    setting: *const c_void,
) -> u32 {
    if setting.is_null() {
        return 0;
    }

    let setting = &*(setting as *const POWERBROADCAST_SETTING);
    if !guid_eq(&setting.PowerSetting, &GUID_ENERGY_SAVER_STATUS) || setting.DataLength < 4 {
        return 0;
    }

    let status = ptr::read_unaligned(setting.Data.as_ptr().cast::<u32>());
    ENERGY_SAVER_STATE.store(status, Ordering::SeqCst);
    crate::debug_log!(
        "energy_saver_callback => type={}, status={}",
        _notification_type,
        status
    );

    let mut ready = ENERGY_SAVER_READY
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    *ready = true;
    ENERGY_SAVER_READY_CVAR.notify_all();
    0
}

pub fn init_energy_saver_tracking() {
    INIT_ENERGY_SAVER.call_once(|| unsafe {
        let params = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(energy_saver_callback),
            Context: ptr::null_mut(),
        };
        let mut registration: *mut c_void = ptr::null_mut();
        let result: WIN32_ERROR = PowerSettingRegisterNotification(
            &GUID_ENERGY_SAVER_STATUS,
            DEVICE_NOTIFY_CALLBACK,
            (&params as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS).cast_mut() as HANDLE,
            &mut registration,
        );
        crate::debug_log!("PowerSettingRegisterNotification => result={}", result);

        if result != 0 {
            return;
        }

        ENERGY_SAVER_REGISTRATION = registration as HPOWERNOTIFY;

        let ready = ENERGY_SAVER_READY
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let _ = ENERGY_SAVER_READY_CVAR
            .wait_timeout_while(ready, ENERGY_SAVER_STATE_QUERY_TIMEOUT, |ready| !*ready)
            .unwrap_or_else(|err| err.into_inner());
    });
}

pub fn shutdown_energy_saver_tracking() {
    unsafe {
        if ENERGY_SAVER_REGISTRATION == 0 {
            return;
        }

        let _result = PowerSettingUnregisterNotification(ENERGY_SAVER_REGISTRATION);
        crate::debug_log!("PowerSettingUnregisterNotification => result={}", _result);
        ENERGY_SAVER_REGISTRATION = 0;
    }
}

pub fn is_energy_saver_active() -> bool {
    let energy_saver_status = ENERGY_SAVER_STATE.load(Ordering::SeqCst);
    if energy_saver_status != ENERGY_SAVER_STATUS_UNKNOWN {
        let active = energy_saver_status != ENERGY_SAVER_STATUS_OFF;
        crate::debug_log!(
            "is_energy_saver_active => status={}, active={}",
            energy_saver_status,
            active
        );
        return active;
    }

    unsafe {
        let mut status: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut status) == 0 {
            crate::debug_log!("GetSystemPowerStatus failed, defaulting to false");
            return false;
        }

        let active = status.SystemStatusFlag == 1;
        crate::debug_log!(
            "is_energy_saver_active => SystemStatusFlag={}, active={}",
            status.SystemStatusFlag,
            active
        );
        active
    }
}

/// Get the current active power mode overlay via Win32 API (dynamic load).
pub fn get_current_mode() -> PowerMode {
    unsafe {
        // Try PowerGetEffectiveOverlayScheme first
        if let Some(func) = get_power_get_effective_overlay() {
            if let Ok(guid) = call_overlay_getter(
                #[cfg(debug_assertions)]
                "PowerGetEffectiveOverlayScheme",
                func,
            ) {
                let mode = PowerMode::from_guid(&guid);
                crate::debug_log!("get_current_mode => {:?}", mode);
                return mode;
            }
        }

        // Fallback to PowerGetActualOverlayScheme
        if let Some(func) = get_power_get_actual_overlay() {
            match call_overlay_getter(
                #[cfg(debug_assertions)]
                "PowerGetActualOverlayScheme",
                func,
            ) {
                Ok(guid) => {
                    let mode = PowerMode::from_guid(&guid);
                    crate::debug_log!("get_current_mode => {:?}", mode);
                    return mode;
                }
                Err(_ret) => {
                    crate::debug_log!(
                        "PowerGetActualOverlayScheme failed (ret={}), defaulting to Balanced",
                        _ret
                    );
                    return PowerMode::Balanced;
                }
            }
        }

        crate::debug_log!("get_current_mode => Balanced");
        PowerMode::Balanced
    }
}

/// Set the power mode overlay via Win32 API (dynamic load).
pub fn set_mode(mode: PowerMode) {
    let guid = mode.to_guid();
    crate::debug_log!("set_mode({:?}) => guid={:?}", mode, guid);

    unsafe {
        if let Some(func) = get_power_set_active_overlay() {
            let ret = call_overlay_setter(
                #[cfg(debug_assertions)]
                "PowerSetActiveOverlayScheme",
                func,
                &guid,
            );
            if ret != 0 {
                crate::debug_log!("WARNING: PowerSetActiveOverlayScheme failed (ret={})", ret);
            }
        }
    }
}
