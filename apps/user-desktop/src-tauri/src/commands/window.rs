/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 baibai and Botting contributors
 *
 * Botting is free software: you can redistribute it and/or modify it under
 * the GNU Affero General Public License version 3, as published by the
 * Free Software Foundation. This program comes WITHOUT ANY WARRANTY;
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
 * PARTICULAR PURPOSE. See the LICENSE file for the complete terms.
 * Copyleft: covered modifications must retain these license obligations.
 * https://www.gnu.org/licenses/agpl-3.0.html
 */

//! 同步調整主視窗與 WebView 的邊界，避免展開動畫期間畫布尺寸落後。

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
/// 驗證主視窗及正尺寸後同步調整原生視窗與 WebView。
/// @param window 提出要求的主視窗。
/// @param x 實體像素左座標。
/// @param y 實體像素上座標。
/// @param width 實體像素寬度。
/// @param height 實體像素高度。
/// @return 視窗與 WebView 更新結果。
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
    // SAFETY: hwnd 來自仍存活的 Tauri 主視窗，current_bounds 指向可寫入的結構。
    let has_current_bounds = unsafe { GetWindowRect(hwnd, &mut current_bounds) } != 0;
    let bounds_changed = !has_current_bounds
        || current_bounds.left != target.x
        || current_bounds.top != target.y
        || current_bounds.right - current_bounds.left != target.width
        || current_bounds.bottom - current_bounds.top != target.height;

    if bounds_changed {
        let controller = webview.controller();
        let mut webview_bounds = Default::default();
        // SAFETY: controller 由 WebView2 持有，邊界先初始化再使用，視窗尺寸已在指令入口驗證。
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
        // SAFETY: controller 屬於剛完成邊界更新且仍存活的同一個 WebView。
        unsafe {
            controller
                .NotifyParentWindowPositionChanged()
                .map_err(|error| error.to_string())?;
        }
    }

    // SAFETY: RedrawWindow 只要求仍存活的主視窗與子視窗重新繪製。
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
