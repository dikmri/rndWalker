//! Puts the embedded executable icon on the window itself, so Windows shows it
//! in the taskbar and Alt-Tab.
//!
//! `ViewportBuilder::with_icon` only reaches winit's `window_icon`, and winit
//! applies that to `ICON_SMALL` (the title bar) alone. The taskbar reads
//! `ICON_BIG`, which winit explicitly clears when its Windows-only
//! `taskbar_icon` attribute is unset — and eframe never sets it. winit also
//! registers its window class with `hIcon: 0`, so there is no fallback to the
//! icon `build.rs` embedded in the .exe: that resource only reaches Explorer.
//!
//! We therefore load the same resource and set both icon slots by hand.

/// Resource name `build.rs` embeds the icon under (the `winresource` default).
#[cfg(windows)]
const ICON_RESOURCE_ID: usize = 1;

#[cfg(windows)]
pub fn apply(cc: &eframe::CreationContext<'_>) {
    use crate::applog::log;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON,
        LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };

    let hwnd: HWND = match cc.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(win32) => win32.hwnd.get() as HWND,
            other => {
                log(
                    "win_icon.apply",
                    &format!("window_handle=non-win32({other:?}) -> skipped"),
                );
                return;
            }
        },
        Err(err) => {
            log("win_icon.apply", &format!("window_handle=Err({err}) -> skipped"));
            return;
        }
    };

    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if hinstance.is_null() {
        log("win_icon.apply", "GetModuleHandleW(null) -> null -> skipped");
        return;
    }

    // `LoadImageW` picks the closest entry in the icon group for the size we
    // ask for, so request the sizes Windows itself uses for each slot.
    let load = |slot: &str, metric_cx, metric_cy| {
        let (cx, cy) = unsafe { (GetSystemMetrics(metric_cx), GetSystemMetrics(metric_cy)) };
        let icon = unsafe {
            LoadImageW(
                hinstance as _,
                ICON_RESOURCE_ID as *const u16,
                IMAGE_ICON,
                cx,
                cy,
                LR_DEFAULTCOLOR,
            )
        };
        log(
            "win_icon.load",
            &format!(
                "slot={slot} resource_id={ICON_RESOURCE_ID} size={cx}x{cy} -> handle={icon:?}"
            ),
        );
        icon
    };

    let big = load("ICON_BIG", SM_CXICON, SM_CYICON);
    let small = load("ICON_SMALL", SM_CXSMICON, SM_CYSMICON);

    for (slot, which, icon) in [
        ("ICON_BIG", ICON_BIG, big),
        ("ICON_SMALL", ICON_SMALL, small),
    ] {
        if icon.is_null() {
            log("win_icon.set", &format!("slot={slot} -> skipped (null icon)"));
            continue;
        }
        let previous =
            unsafe { SendMessageW(hwnd, WM_SETICON, which as WPARAM, icon as LPARAM) };
        log(
            "win_icon.set",
            &format!("slot={slot} hwnd={hwnd:?} icon={icon:?} -> previous={previous}"),
        );
    }
}

#[cfg(not(windows))]
pub fn apply(_cc: &eframe::CreationContext<'_>) {}
