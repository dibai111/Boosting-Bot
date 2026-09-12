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

//! 在專用執行緒接收 Windows 低階鍵盤事件，忽略注入按鍵並抑制長按重複觸發。

use super::{ShortcutPair, ShortcutSpec, ALT, CONTROL, SHIFT, SUPER};
use std::{
    io,
    mem::zeroed,
    ptr,
    sync::{mpsc, Mutex, OnceLock},
    thread,
};
use windows_sys::Win32::{
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
            LLKHF_ALTDOWN, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
            WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

struct HookState {
    shortcuts: ShortcutPair,
    sender: mpsc::SyncSender<&'static str>,
    stop_pressed: bool,
    overlay_pressed: bool,
}

static STATE: OnceLock<Mutex<HookState>> = OnceLock::new();

/// 建立低階鍵盤 hook，等待安裝結果後才返回。
/// @param shortcuts 初始快捷鍵規格。
/// @param sender 發送操作名稱的有界同步通道。
/// @return 安裝結果；重複啟動會回傳錯誤。
pub(super) fn start(
    shortcuts: ShortcutPair,
    sender: mpsc::SyncSender<&'static str>,
) -> io::Result<()> {
    STATE
        .set(Mutex::new(HookState {
            shortcuts,
            sender,
            stop_pressed: false,
            overlay_pressed: false,
        }))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "keyboard hook already started",
            )
        })?;

    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("matchmaking-keyboard-hook".to_owned())
        .spawn(move || run_hook(ready_sender))?;
    ready_receiver
        .recv()
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "keyboard hook stopped"))?
}

/// 持鎖更新快捷鍵並清除長按追蹤。
/// @param shortcuts 新的配對快捷鍵組。
/// @return 更新結果；hook 未啟動或鎖失敗時為 Err。
pub(super) fn configure(shortcuts: ShortcutPair) -> Result<(), ()> {
    let mut state = STATE.get().ok_or(())?.lock().map_err(|_| ())?;
    state.shortcuts = shortcuts;
    state.stop_pressed = false;
    state.overlay_pressed = false;
    Ok(())
}

fn run_hook(ready: mpsc::SyncSender<io::Result<()>>) {
    // SAFETY: 回呼具有固定函式位址，GetModuleHandleW 取得目前程序模組，hook 類型與回呼簽章相符。
    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_callback),
            GetModuleHandleW(ptr::null()),
            0,
        )
    };
    if hook.is_null() {
        let _ = ready.send(Err(io::Error::last_os_error()));
        return;
    }
    if ready.send(Ok(())).is_err() {
        // SAFETY: hook 來自成功的 SetWindowsHookExW，且尚未解除。
        unsafe { UnhookWindowsHookEx(hook) };
        return;
    }

    // SAFETY: MSG 允許以全零初始化，之後由 GetMessageW 填入訊息內容。
    let mut message: MSG = unsafe { zeroed() };
    // SAFETY: message 指向本執行緒的可寫入結構，空視窗 handle 表示取得此執行緒的訊息。
    while unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } > 0 {}
    // SAFETY: hook 仍處於安裝狀態，於訊息迴圈結束後解除一次。
    unsafe { UnhookWindowsHookEx(hook) };
}

unsafe extern "system" fn keyboard_callback(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        // SAFETY: 非負的低階鍵盤 hook 代碼保證 lparam 指向有效的 KBDLLHOOKSTRUCT。
        let event = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        // 忽略程式注入的鍵盤事件，避免合成按鍵再次觸發全域快捷鍵。
        if event.flags & LLKHF_INJECTED == 0 {
            if wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize {
                handle_key_down(event);
            } else if wparam == WM_KEYUP as usize || wparam == WM_SYSKEYUP as usize {
                handle_key_up(event.vkCode);
            }
        }
    }
    // SAFETY: 沿用 Windows 提供的原始參數轉送事件，低階 hook 允許傳入空的 hook handle。
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

fn handle_key_down(event: &KBDLLHOOKSTRUCT) {
    let modifiers = current_modifiers(event.flags & LLKHF_ALTDOWN != 0);
    let Some(state) = STATE.get() else { return };
    let Ok(mut state) = state.lock() else { return };

    if matches_shortcut(state.shortcuts.stop, modifiers, event.vkCode) {
        if !state.stop_pressed {
            state.stop_pressed = true;
            let _ = state.sender.try_send("stop");
        }
    } else if matches_shortcut(state.shortcuts.show_overlay, modifiers, event.vkCode)
        && !state.overlay_pressed
    {
        state.overlay_pressed = true;
        let _ = state.sender.try_send("show_overlay");
    }
}

fn handle_key_up(virtual_key: u32) {
    let Some(state) = STATE.get() else { return };
    let Ok(mut state) = state.lock() else { return };
    if state.shortcuts.stop.virtual_key == virtual_key {
        state.stop_pressed = false;
    }
    if state.shortcuts.show_overlay.virtual_key == virtual_key {
        state.overlay_pressed = false;
    }
}

fn matches_shortcut(shortcut: ShortcutSpec, modifiers: u8, virtual_key: u32) -> bool {
    shortcut.virtual_key == virtual_key && shortcut.modifiers == modifiers
}

fn current_modifiers(alt_down: bool) -> u8 {
    let mut modifiers = 0;
    if is_key_down(VK_CONTROL) {
        modifiers |= CONTROL;
    }
    if alt_down || is_key_down(VK_MENU) {
        modifiers |= ALT;
    }
    if is_key_down(VK_SHIFT) {
        modifiers |= SHIFT;
    }
    if is_key_down(VK_LWIN) || is_key_down(VK_RWIN) {
        modifiers |= SUPER;
    }
    modifiers
}

fn is_key_down(virtual_key: u16) -> bool {
    // SAFETY: virtual_key 是 Windows 虛擬按鍵代碼，此 API 不接收記憶體指標。
    (unsafe { GetAsyncKeyState(i32::from(virtual_key)) }) < 0
}
