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

pub(super) fn configure(shortcuts: ShortcutPair) -> Result<(), ()> {
    let mut state = STATE.get().ok_or(())?.lock().map_err(|_| ())?;
    state.shortcuts = shortcuts;
    state.stop_pressed = false;
    state.overlay_pressed = false;
    Ok(())
}

fn run_hook(ready: mpsc::SyncSender<io::Result<()>>) {
    // SAFETY: The callback has static function linkage, the null module handle requests this
    // process module, and the hook id/callback pair are valid for a low-level keyboard hook.
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
        // SAFETY: hook was returned by SetWindowsHookExW and has not been unhooked yet.
        unsafe { UnhookWindowsHookEx(hook) };
        return;
    }

    // SAFETY: MSG is a plain Windows message structure for which an all-zero initial value is
    // valid before GetMessageW fills it.
    let mut message: MSG = unsafe { zeroed() };
    // SAFETY: message points to writable storage owned by this thread; a null window handle asks
    // Windows to retrieve messages for the current thread.
    while unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } > 0 {}
    // SAFETY: hook was returned by SetWindowsHookExW and remains installed until this point.
    unsafe { UnhookWindowsHookEx(hook) };
}

unsafe extern "system" fn keyboard_callback(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        // SAFETY: Windows provides lParam as a pointer to a KBDLLHOOKSTRUCT for a non-negative
        // low-level keyboard hook code.
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
    // SAFETY: The callback forwards the values supplied by Windows unchanged; a null hook handle
    // is permitted when forwarding a low-level hook event.
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
    // SAFETY: virtual_key is a Windows virtual-key code and the API has no pointer arguments.
    (unsafe { GetAsyncKeyState(i32::from(virtual_key)) }) < 0
}
