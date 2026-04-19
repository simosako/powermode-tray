use std::ptr;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, SetForegroundWindow, TrackPopupMenu,
    HMENU, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
};

use crate::power::{self, PowerMode};
use crate::util::to_wide;

// Menu item IDs
pub const IDM_ABOUT: u32 = 2000;
pub const IDM_QUIT: u32 = 2001;

/// Show the context menu at the current cursor position.
/// Returns when the user selects an item or dismisses the menu.
pub fn show_context_menu(hwnd: HWND) {
    let hmenu = unsafe { CreatePopupMenu() };
    if hmenu == 0 as HMENU {
        return;
    }

    let current = power::get_current_mode();
    let energy_saver_active = power::is_energy_saver_active();

    if energy_saver_active {
        let status_label = to_wide("Energy saver active");
        unsafe {
            AppendMenuW(hmenu, MF_STRING | MF_GRAYED, 0, status_label.as_ptr());
            AppendMenuW(hmenu, MF_SEPARATOR, 0, ptr::null());
        }
    }

    // Keep the tray menu order independent from the enum declaration order.
    const MENU_ORDER: [PowerMode; 3] = [
        PowerMode::BestPerformance,
        PowerMode::Balanced,
        PowerMode::BestPowerEfficiency,
    ];

    // Add power mode items (check mark on current mode)
    for mode in MENU_ORDER {
        let mut flags = MF_STRING;
        if mode == current {
            flags |= MF_CHECKED;
        }
        if energy_saver_active {
            flags |= MF_GRAYED;
        }
        let wide_label = to_wide(mode.label());
        unsafe {
            AppendMenuW(
                hmenu,
                flags,
                mode.to_menu_id() as usize,
                wide_label.as_ptr(),
            );
        }
    }

    // Separator
    unsafe {
        AppendMenuW(hmenu, MF_SEPARATOR, 0, ptr::null());
    }

    // About
    let about_label = to_wide("About");
    unsafe {
        AppendMenuW(hmenu, MF_STRING, IDM_ABOUT as usize, about_label.as_ptr());
    }

    // Quit
    let quit_label = to_wide("Quit");
    unsafe {
        AppendMenuW(hmenu, MF_STRING, IDM_QUIT as usize, quit_label.as_ptr());
    }

    // Get cursor position
    let mut pt: POINT = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut pt);
    }

    // Required to make the menu dismiss properly when clicking outside
    unsafe {
        SetForegroundWindow(hwnd);
    }

    unsafe {
        TrackPopupMenu(
            hmenu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            ptr::null(),
        );
    }

    unsafe {
        DestroyMenu(hmenu);
    }
}
