use std::mem;
use std::ptr;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, LoadIconW, LoadImageW, RegisterClassW,
    CW_USEDEFAULT, HMENU, IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, WM_USER,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

use crate::power::PowerMode;
use crate::util::{to_wide, to_wide_array};

/// Custom message ID for tray icon events (WM_USER + 1)
pub const WM_TRAY_ICON: u32 = WM_USER + 1;

/// Tray icon ID
const TRAY_ICON_ID: u32 = 1;
const IDI_POWERMODE_BALANCED: u16 = 101;
const IDI_POWERMODE_PERFORMANCE: u16 = 102;
const IDI_POWERMODE_EFFICIENCY: u16 = 103;

fn mode_icon_resource_id(mode: PowerMode) -> u16 {
    match mode {
        PowerMode::Balanced => IDI_POWERMODE_BALANCED,
        PowerMode::BestPerformance => IDI_POWERMODE_PERFORMANCE,
        PowerMode::BestPowerEfficiency => IDI_POWERMODE_EFFICIENCY,
    }
}

fn load_mode_icon(mode: PowerMode) -> *mut core::ffi::c_void {
    let hinstance = unsafe { GetModuleHandleW(ptr::null()) };
    let resource_id = mode_icon_resource_id(mode) as usize as *const u16;
    let hicon: *mut core::ffi::c_void = unsafe {
        LoadImageW(
            hinstance,
            resource_id,
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        ) as _
    };

    if hicon.is_null() {
        crate::debug_log!(
            "Failed to load tray icon resource for {:?}, using fallback",
            mode
        );
        return unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) as _ };
    }

    hicon
}

fn tray_tooltip(mode: PowerMode) -> [u16; 128] {
    to_wide_array::<128>(&format!("Power Mode Tray - {}", mode.label()))
}

fn tray_icon_data(hwnd: HWND, mode: PowerMode) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { mem::zeroed() };
    nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY_ICON;
    nid.hIcon = load_mode_icon(mode);
    nid.szTip = tray_tooltip(mode);
    nid
}

fn notify_tray_icon(message: u32, hwnd: HWND, mode: PowerMode) {
    let nid = tray_icon_data(hwnd, mode);
    unsafe {
        Shell_NotifyIconW(message, &nid);
    }
}

/// Create a hidden top-level window for message handling and return its HWND.
/// `wnd_proc` is the window procedure that handles messages.
pub fn create_hidden_window(
    wnd_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) -> HWND {
    let hinstance = unsafe { GetModuleHandleW(ptr::null()) };
    let class_name = to_wide("PowerModeTrayClass");

    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };

    let atom = unsafe { RegisterClassW(&wc) };
    if atom == 0 {
        crate::debug_log!("RegisterClassW failed");
        return ptr::null_mut();
    }

    // Bind window title to a variable so the Vec lives long enough
    // for CreateWindowExW to read from the pointer.
    let window_title = to_wide("PowerModeTray");

    unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0 as HWND,
            0 as HMENU,
            hinstance,
            ptr::null(),
        )
    }
}

/// Add a tray icon to the system tray.
pub fn add_tray_icon(hwnd: HWND, mode: PowerMode) {
    notify_tray_icon(NIM_ADD, hwnd, mode);
}

/// Update the tray icon to match the current power mode.
pub fn update_tray_icon(hwnd: HWND, mode: PowerMode) {
    notify_tray_icon(NIM_MODIFY, hwnd, mode);
}

/// Remove the tray icon from the system tray.
pub fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { mem::zeroed() };
    nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;

    let deleted = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
    if deleted == 0 {
        crate::debug_log!("Shell_NotifyIconW(NIM_DELETE) failed");
    }
}

/// Destroy the hidden window.
pub fn destroy_window(hwnd: HWND) {
    unsafe {
        DestroyWindow(hwnd);
    }
}

/// Default window procedure passthrough.
pub fn default_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
