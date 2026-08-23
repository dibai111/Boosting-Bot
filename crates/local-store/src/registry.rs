use anyhow::{bail, Context, Result};

pub(crate) const DEFAULT_PATH: &str = "Software\\BoostingBot";
pub(crate) const STATE_VALUE: &str = "State";
pub(crate) const MAX_VALUE_BYTES: usize = 512 * 1024;

#[cfg(windows)]
use std::ptr;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA},
    System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_BINARY, REG_OPTION_NON_VOLATILE,
    },
};

pub(crate) fn ensure_key(path: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let _ = open_key(path, true)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        bail!("encrypted local storage is only supported on Windows")
    }
}

pub(crate) fn read_value(path: &str) -> Result<Option<Vec<u8>>> {
    #[cfg(windows)]
    {
        let key = open_key(path, false)?;
        let value_name = wide(STATE_VALUE);
        let mut value_type = 0;
        let mut byte_count = 0;
        // SAFETY: key and value_name are valid for the duration of the call; null data only asks
        // Windows for the required size.
        let result = unsafe {
            RegQueryValueExW(
                key.0,
                value_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                ptr::null_mut(),
                &mut byte_count,
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if result != 0 {
            bail!("read Registry state size: {result}")
        }
        if value_type != REG_BINARY {
            bail!("Registry state has an unexpected value type")
        }
        let length = usize::try_from(byte_count).context("invalid Registry state length")?;
        if length > MAX_VALUE_BYTES {
            bail!("Registry state is larger than the supported limit")
        }
        let mut bytes = vec![0u8; length];
        // SAFETY: key/value_name remain valid and bytes has exactly the requested writable size.
        let result = unsafe {
            RegQueryValueExW(
                key.0,
                value_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                bytes.as_mut_ptr(),
                &mut byte_count,
            )
        };
        if result == ERROR_MORE_DATA {
            bail!("Registry state changed while it was being read")
        }
        if result != 0 {
            bail!("read Registry state: {result}")
        }
        let used = usize::try_from(byte_count).context("invalid Registry state byte count")?;
        bytes.truncate(used);
        Ok(Some(bytes))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        bail!("encrypted local storage is only supported on Windows")
    }
}

pub(crate) fn write_value(path: &str, bytes: &[u8]) -> Result<()> {
    #[cfg(windows)]
    {
        if bytes.len() > MAX_VALUE_BYTES {
            bail!("Registry state is larger than the supported limit")
        }
        let byte_count = u32::try_from(bytes.len()).context("Registry state is too large")?;
        let key = open_key(path, true)?;
        let value_name = wide(STATE_VALUE);
        // SAFETY: key/value_name are valid and bytes remains alive for the duration of the call.
        let result = unsafe {
            RegSetValueExW(
                key.0,
                value_name.as_ptr(),
                0,
                REG_BINARY,
                bytes.as_ptr(),
                byte_count,
            )
        };
        if result != 0 {
            bail!("write Registry state: {result}")
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (path, bytes);
        bail!("encrypted local storage is only supported on Windows")
    }
}

#[cfg(test)]
pub(crate) fn delete_key(path: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::RegDeleteTreeW;
        let subkey = wide(path);
        // SAFETY: HKEY_CURRENT_USER and the NUL-terminated subkey are valid Windows handles.
        let result = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, subkey.as_ptr()) };
        if result != 0 && result != ERROR_FILE_NOT_FOUND {
            bail!("delete test Registry key: {result}")
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(windows)]
fn open_key(path: &str, create: bool) -> Result<RegistryKey> {
    let subkey = wide(path);
    let mut key = ptr::null_mut();
    let result = if create {
        // SAFETY: all pointers refer to valid NUL-terminated strings or writable local storage;
        // the requested key is scoped to the current Windows user.
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_READ | KEY_WRITE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        }
    } else {
        // SAFETY: HKEY_CURRENT_USER and the NUL-terminated subkey are valid; key is writable.
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut key) }
    };
    if result != 0 {
        bail!("open Registry key {path}: {result}")
    }
    Ok(RegistryKey(key))
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
struct RegistryKey(HKEY);

#[cfg(windows)]
impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by a successful Registry open/create call and is closed
        // exactly once when this RAII guard is dropped.
        unsafe { RegCloseKey(self.0) };
    }
}
