#[cfg(target_os = "windows")]
mod windows_hook;

use std::{io, sync::mpsc, thread};
use tauri::{AppHandle, Emitter};

const CONTROL: u8 = 1 << 0;
const ALT: u8 = 1 << 1;
const SHIFT: u8 = 1 << 2;
const SUPER: u8 = 1 << 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ShortcutSpec {
    pub(super) modifiers: u8,
    pub(super) virtual_key: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ShortcutPair {
    pub(super) stop: ShortcutSpec,
    pub(super) show_overlay: ShortcutSpec,
}

const DEFAULT_SHORTCUTS: ShortcutPair = ShortcutPair {
    stop: ShortcutSpec {
        modifiers: 0,
        virtual_key: 0x77,
    },
    show_overlay: ShortcutSpec {
        modifiers: 0,
        virtual_key: 0x78,
    },
};

pub(crate) struct MatchmakingShortcuts;

impl MatchmakingShortcuts {
    pub(crate) fn configure(&self, stop: &str, show_overlay: &str) -> Result<(), String> {
        let pair = ShortcutPair {
            stop: parse_shortcut(stop)?,
            show_overlay: parse_shortcut(show_overlay)?,
        };
        if pair.stop == pair.show_overlay {
            return Err("shortcut_conflict".to_owned());
        }

        update_hook(pair).map_err(|_| "shortcut_state_unavailable".to_owned())
    }
}

pub(crate) fn start(app: AppHandle) -> io::Result<()> {
    let (sender, receiver) = mpsc::sync_channel(16);
    start_hook(DEFAULT_SHORTCUTS, sender)?;
    thread::Builder::new()
        .name("matchmaking-shortcut-events".to_owned())
        .spawn(move || {
            while let Ok(action) = receiver.recv() {
                let _ = app.emit("matchmaking-shortcut", action);
            }
        })?;
    Ok(())
}

fn parse_shortcut(value: &str) -> Result<ShortcutSpec, String> {
    let mut modifiers = 0;
    let mut virtual_key = None;

    for part in value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match part.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => modifiers |= CONTROL,
            "alt" => modifiers |= ALT,
            "shift" => modifiers |= SHIFT,
            "super" | "win" | "meta" => modifiers |= SUPER,
            _ if virtual_key.is_none() => virtual_key = parse_virtual_key(part),
            _ => return Err("shortcut_invalid".to_owned()),
        }
    }

    let virtual_key = virtual_key.ok_or_else(|| "shortcut_invalid".to_owned())?;
    let is_letter = u32::from(b'A') <= virtual_key && virtual_key <= u32::from(b'Z');
    let is_function_key = (0x70..=0x87).contains(&virtual_key);
    if modifiers == 0 && !is_letter && !is_function_key {
        return Err("shortcut_modifier_required".to_owned());
    }
    Ok(ShortcutSpec {
        modifiers,
        virtual_key,
    })
}

fn parse_virtual_key(value: &str) -> Option<u32> {
    let upper = value.to_ascii_uppercase();
    if let Some(letter) = upper.strip_prefix("KEY") {
        return single_ascii(letter, b'A', b'Z');
    }
    if let Some(digit) = upper.strip_prefix("DIGIT") {
        return single_ascii(digit, b'0', b'9');
    }
    if let Some(number) = upper
        .strip_prefix('F')
        .and_then(|value| value.parse::<u32>().ok())
    {
        return (1..=24).contains(&number).then_some(0x70 + number - 1);
    }
    if let Some(number) = upper
        .strip_prefix("NUMPAD")
        .and_then(|value| value.parse::<u32>().ok())
    {
        return (number <= 9).then_some(0x60 + number);
    }

    Some(match upper.as_str() {
        "BACKSPACE" => 0x08,
        "TAB" => 0x09,
        "ENTER" | "NUMPADENTER" => 0x0D,
        "SPACE" => 0x20,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "END" => 0x23,
        "HOME" => 0x24,
        "ARROWLEFT" => 0x25,
        "ARROWUP" => 0x26,
        "ARROWRIGHT" => 0x27,
        "ARROWDOWN" => 0x28,
        "INSERT" => 0x2D,
        "DELETE" => 0x2E,
        "NUMPADMULTIPLY" => 0x6A,
        "NUMPADADD" => 0x6B,
        "NUMPADSUBTRACT" => 0x6D,
        "NUMPADDECIMAL" => 0x6E,
        "NUMPADDIVIDE" => 0x6F,
        "SEMICOLON" => 0xBA,
        "EQUAL" => 0xBB,
        "COMMA" => 0xBC,
        "MINUS" => 0xBD,
        "PERIOD" => 0xBE,
        "SLASH" => 0xBF,
        "BACKQUOTE" => 0xC0,
        "BRACKETLEFT" => 0xDB,
        "BACKSLASH" => 0xDC,
        "BRACKETRIGHT" => 0xDD,
        "QUOTE" => 0xDE,
        _ => return None,
    })
}

fn single_ascii(value: &str, start: u8, end: u8) -> Option<u32> {
    let bytes = value.as_bytes();
    (bytes.len() == 1 && (start..=end).contains(&bytes[0])).then_some(u32::from(bytes[0]))
}

#[cfg(target_os = "windows")]
fn start_hook(shortcuts: ShortcutPair, sender: mpsc::SyncSender<&'static str>) -> io::Result<()> {
    windows_hook::start(shortcuts, sender)
}

#[cfg(not(target_os = "windows"))]
fn start_hook(_shortcuts: ShortcutPair, _sender: mpsc::SyncSender<&'static str>) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "keyboard shortcuts require Windows",
    ))
}

#[cfg(target_os = "windows")]
fn update_hook(shortcuts: ShortcutPair) -> Result<(), ()> {
    windows_hook::configure(shortcuts)
}

#[cfg(not(target_os = "windows"))]
fn update_hook(_shortcuts: ShortcutPair) -> Result<(), ()> {
    Err(())
}

#[cfg(test)]
mod tests {
    use super::{parse_shortcut, ShortcutSpec, ALT, CONTROL, DEFAULT_SHORTCUTS, SHIFT};

    #[test]
    fn default_shortcuts_use_unmodified_function_keys() {
        assert_eq!(DEFAULT_SHORTCUTS.stop.modifiers, 0);
        assert_eq!(DEFAULT_SHORTCUTS.stop.virtual_key, 0x77);
        assert_eq!(DEFAULT_SHORTCUTS.show_overlay.modifiers, 0);
        assert_eq!(DEFAULT_SHORTCUTS.show_overlay.virtual_key, 0x78);
    }

    #[test]
    fn parses_modifier_combinations_and_function_keys() {
        assert_eq!(
            parse_shortcut("control+shift+KeyM").unwrap(),
            ShortcutSpec {
                modifiers: CONTROL | SHIFT,
                virtual_key: u32::from(b'M'),
            }
        );
        assert_eq!(
            parse_shortcut("F12").unwrap(),
            ShortcutSpec {
                modifiers: 0,
                virtual_key: 0x7B,
            }
        );
        assert_eq!(
            parse_shortcut("alt+Digit7").unwrap(),
            ShortcutSpec {
                modifiers: ALT,
                virtual_key: u32::from(b'7'),
            }
        );
    }

    #[test]
    fn accepts_letters_alone_and_in_combinations() {
        assert_eq!(
            parse_shortcut("KeyM").unwrap(),
            ShortcutSpec {
                modifiers: 0,
                virtual_key: u32::from(b'M'),
            }
        );
        assert_eq!(
            parse_shortcut("control+alt+KeyZ").unwrap(),
            ShortcutSpec {
                modifiers: CONTROL | ALT,
                virtual_key: u32::from(b'Z'),
            }
        );
    }

    #[test]
    fn still_rejects_unmodified_non_letter_keys() {
        assert_eq!(
            parse_shortcut("Digit7").unwrap_err(),
            "shortcut_modifier_required"
        );
    }
}
