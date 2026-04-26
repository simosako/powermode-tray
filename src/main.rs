#![windows_subsystem = "windows"]

mod debug;
mod menu;
mod power;
mod tray;
mod util;

use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, MessageBoxIndirectW, PostQuitMessage, SetTimer,
    TranslateMessage, MSG, MSGBOXPARAMSW, MB_OK, MB_USERICON, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    WM_ENDSESSION, WM_LBUTTONUP, WM_QUERYENDSESSION, WM_RBUTTONUP, WM_TIMER,
};

use menu::{IDM_ABOUT, IDM_QUIT};
use power::PowerMode;
use tray::WM_TRAY_ICON;
use util::to_wide;
pub(crate) use debug::debug_log;

const APP_NAME: &str = "powermode-tray";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const MODE_POLL_TIMER_ID: usize = 1;
const MODE_POLL_INTERVAL_MS: u32 = 1000;
const MODE_UNINITIALIZED: u32 = u32::MAX;

static ABOUT_DIALOG_OPEN: AtomicBool = AtomicBool::new(false);
static LAST_DISPLAYED_MODE: AtomicU32 = AtomicU32::new(MODE_UNINITIALIZED);
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

fn store_displayed_mode(mode: PowerMode) {
    LAST_DISPLAYED_MODE.store(mode.to_stored_u32(), Ordering::SeqCst);
}

fn displayed_mode() -> Option<PowerMode> {
    PowerMode::from_stored_u32(LAST_DISPLAYED_MODE.load(Ordering::SeqCst))
}

fn sync_tray_mode(hwnd: HWND, mode: PowerMode) {
    tray::update_tray_icon(hwnd, mode);
    store_displayed_mode(mode);
    debug_log!("Tray icon updated for mode: {:?}", mode);
}

fn initialize_tray_mode(hwnd: HWND, mode: PowerMode) {
    tray::add_tray_icon(hwnd, mode);
    store_displayed_mode(mode);
    debug_log!("Tray icon added for mode: {:?}", mode);
}

/// Initiate graceful shutdown by destroying the window.
/// All resource cleanup (timer, tray icon, energy saver tracking) is
/// handled in the WM_DESTROY handler to keep teardown in one place.
fn request_shutdown(hwnd: HWND) {
    if SHUTDOWN_REQUESTED.swap(true, Ordering::SeqCst) {
        return;
    }

    tray::destroy_window(hwnd);
}

fn show_about_dialog(hwnd: HWND) {
    if ABOUT_DIALOG_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }

    let title = to_wide("About");
    let body = to_wide(&format!("{}\r\nVersion {}", APP_NAME, APP_VERSION));
    let hinstance = unsafe { GetModuleHandleW(ptr::null()) };
    let icon_id = tray::APP_ICON_RESOURCE_ID as usize as *const u16;
    let params = MSGBOXPARAMSW {
        cbSize: std::mem::size_of::<MSGBOXPARAMSW>() as u32,
        hwndOwner: hwnd,
        hInstance: hinstance,
        lpszText: body.as_ptr(),
        lpszCaption: title.as_ptr(),
        dwStyle: MB_OK | MB_USERICON,
        lpszIcon: icon_id,
        dwContextHelpId: 0,
        lpfnMsgBoxCallback: None,
        dwLanguageId: 0,
    };

    unsafe {
        MessageBoxIndirectW(&params);
    }
    ABOUT_DIALOG_OPEN.store(false, Ordering::SeqCst);
}

// ── Window procedure ───────────────────────────────────────────

/// Window procedure: handles tray icon events and menu commands.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY_ICON => {
            // lparam contains the mouse message
            let mouse_msg = (lparam & 0xFFFF) as u32;
            if mouse_msg == WM_RBUTTONUP || mouse_msg == WM_LBUTTONUP {
                debug_log!("Tray icon clicked, showing context menu");
                menu::show_context_menu(hwnd);
            }
            0
        }
        WM_COMMAND => {
            let cmd_id = (wparam & 0xFFFF) as u32;
            debug_log!("WM_COMMAND received: cmd_id={}", cmd_id);
            if cmd_id == IDM_ABOUT {
                debug_log!("About requested");
                show_about_dialog(hwnd);
            } else if cmd_id == IDM_QUIT {
                debug_log!("Quit requested");
                request_shutdown(hwnd);
            } else if let Some(mode) = PowerMode::from_menu_id(cmd_id) {
                debug_log!("Mode change requested: {:?}", mode);
                power::set_mode(mode);
                let current_mode = power::get_current_mode();
                sync_tray_mode(hwnd, current_mode);
            }
            0
        }
        WM_CLOSE => {
            debug_log!("WM_CLOSE received");
            request_shutdown(hwnd);
            0
        }
        WM_QUERYENDSESSION => {
            debug_log!("WM_QUERYENDSESSION received");
            1
        }
        WM_ENDSESSION => {
            let session_ending = wparam != 0;
            debug_log!("WM_ENDSESSION received: ending={}", session_ending);
            if session_ending {
                request_shutdown(hwnd);
            }
            0
        }
        WM_TIMER => {
            if wparam == MODE_POLL_TIMER_ID {
                let current_mode = power::get_current_mode();
                if displayed_mode() != Some(current_mode) {
                    sync_tray_mode(hwnd, current_mode);
                }
                return 0;
            }

            tray::default_proc(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            debug_log!("WM_DESTROY — shutting down");
            KillTimer(hwnd, MODE_POLL_TIMER_ID);
            tray::remove_tray_icon(hwnd);
            power::shutdown_energy_saver_tracking();
            PostQuitMessage(0);
            0
        }
        _ => tray::default_proc(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    debug_log!("=== powermode-tray starting ===");

    unsafe {
        let hwnd = tray::create_hidden_window(wnd_proc);
        if hwnd.is_null() {
            debug_log!("Failed to create hidden window");
            return;
        }
        debug_log!("Hidden window created: {:?}", hwnd);

        power::init_energy_saver_tracking();
        let initial_mode = power::get_current_mode();
        debug_log!("Initial power mode: {:?}", initial_mode);

        initialize_tray_mode(hwnd, initial_mode);

        SetTimer(hwnd, MODE_POLL_TIMER_ID, MODE_POLL_INTERVAL_MS, None);

        // Message loop
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        debug_log!("=== powermode-tray exiting ===");
    }
}
