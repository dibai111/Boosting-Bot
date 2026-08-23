use crate::app_state::CommandResult;
const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Clone, Copy)]
struct WindowBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[tauri::command]
pub(crate) async fn set_main_window_layout(
    window: tauri::WebviewWindow,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> CommandResult<()> {
    if window.label() != MAIN_WINDOW_LABEL {
        return Err("window bounds can only be changed for the main window".into());
    }
    if width <= 0 || height <= 0 {
        return Err("window dimensions must be positive".into());
    }
    let target = WindowBounds {
        x,
        y,
        width,
        height,
    };

    set_platform_window_layout(window, target).await
}

#[cfg(windows)]
async fn set_platform_window_layout(
    window: tauri::WebviewWindow,
    target: WindowBounds,
) -> CommandResult<()> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as usize;
    let (sender, receiver) = tokio::sync::oneshot::channel();

    window
        .with_webview(move |webview| {
            let result = set_windows_webview_layout(webview, hwnd, target);
            let _ = sender.send(result);
        })
        .map_err(|error| error.to_string())?;

    receiver
        .await
        .map_err(|_| "window closed before its bounds could be changed".to_string())?
}

#[cfg(windows)]
fn set_windows_webview_layout(
    webview: tauri::webview::PlatformWebview,
    hwnd: usize,
    target: WindowBounds,
) -> CommandResult<()> {
    use std::{io, ptr};
    use windows_sys::Win32::Graphics::Gdi::{
        RedrawWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RDW_UPDATENOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOOWNERZORDER,
        SWP_NOZORDER,
    };

    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    let mut current_bounds = Default::default();
    // SAFETY: `hwnd` comes from Tauri's live main window and `current_bounds` is writable.
    let has_current_bounds = unsafe { GetWindowRect(hwnd, &mut current_bounds) } != 0;
    let bounds_changed = !has_current_bounds
        || current_bounds.left != target.x
        || current_bounds.top != target.y
        || current_bounds.right - current_bounds.left != target.width
        || current_bounds.bottom - current_bounds.top != target.height;

    if bounds_changed {
        let controller = webview.controller();
        let mut webview_bounds = Default::default();
        // SAFETY: WebView2 owns the controller, the bounds are initialized before use, and the
        // native window coordinates were validated by the command boundary.
        let succeeded = unsafe {
            controller
                .Bounds(&mut webview_bounds)
                .map_err(|error| error.to_string())?;
            webview_bounds.left = 0;
            webview_bounds.top = 0;
            webview_bounds.right = target.width;
            webview_bounds.bottom = target.height;
            controller
                .SetBounds(webview_bounds)
                .map_err(|error| error.to_string())?;

            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                target.x,
                target.y,
                target.width,
                target.height,
                SWP_NOCOPYBITS | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
        };
        if succeeded == 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        // SAFETY: the controller belongs to the same live WebView whose bounds were updated.
        unsafe {
            controller
                .NotifyParentWindowPositionChanged()
                .map_err(|error| error.to_string())?;
        }
    }

    // SAFETY: RedrawWindow only invalidates the live main window and its children.
    unsafe {
        RedrawWindow(
            hwnd,
            ptr::null(),
            ptr::null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
        );
    }
    Ok(())
}

#[cfg(not(windows))]
async fn set_platform_window_layout(
    window: tauri::WebviewWindow,
    target: WindowBounds,
) -> CommandResult<()> {
    window
        .set_position(tauri::PhysicalPosition::new(target.x, target.y))
        .map_err(|error| error.to_string())?;
    window
        .set_size(tauri::PhysicalSize::new(
            u32::try_from(target.width).map_err(|_| "window width must be positive")?,
            u32::try_from(target.height).map_err(|_| "window height must be positive")?,
        ))
        .map_err(|error| error.to_string())
}
