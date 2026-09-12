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

//! 以 RAII 管理 Registry handle，限制二進位資料大小並回報讀取期間的變更。

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

/// 確保目前使用者的 Registry 子鍵存在。
/// @param path 相對於 HKEY_CURRENT_USER 的子鍵路徑。
/// @return 開啟或建立結果；非 Windows 回傳不支援。
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

/// 限制大小與資料型別後讀取 State 值。
/// @param path 目前使用者的儲存子鍵路徑。
/// @return 二進位值；未設定時為 None，讀取期間增大時回傳錯誤。
pub(crate) fn read_value(path: &str) -> Result<Option<Vec<u8>>> {
    #[cfg(windows)]
    {
        let key = open_key(path, false)?;
        let value_name = wide(STATE_VALUE);
        let mut value_type = 0;
        let mut byte_count = 0;
        // SAFETY: key 與以 NUL 結尾的名稱在呼叫期間有效；空的資料指標僅用於查詢所需大小。
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
        // SAFETY: key 與名稱仍有效，bytes 具有前一次查詢要求的可寫入空間。
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

/// 將加密位元組以 REG_BINARY 寫入 State 值。
/// @param path 目前使用者的儲存子鍵路徑。
/// @param bytes 已加密且不超過 MAX_VALUE_BYTES 的資料。
/// @return Registry 寫入結果。
pub(crate) fn write_value(path: &str, bytes: &[u8]) -> Result<()> {
    #[cfg(windows)]
    {
        if bytes.len() > MAX_VALUE_BYTES {
            bail!("Registry state is larger than the supported limit")
        }
        let byte_count = u32::try_from(bytes.len()).context("Registry state is too large")?;
        let key = open_key(path, true)?;
        let value_name = wide(STATE_VALUE);
        // SAFETY: key、名稱及 bytes 在整次寫入呼叫期間保持有效。
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
        // SAFETY: 使用有效的 HKEY_CURRENT_USER 與以 NUL 結尾的測試子鍵路徑。
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
/// 以所需存取權開啟或建立 Registry 子鍵。
/// @param path 相對於 HKEY_CURRENT_USER 的子鍵路徑。
/// @param create true 時建立子鍵並取得寫入權限。
/// @return 會自動關閉 handle 的 RAII 守衛。
fn open_key(path: &str, create: bool) -> Result<RegistryKey> {
    let subkey = wide(path);
    let mut key = ptr::null_mut();
    let result = if create {
        // SAFETY: 字串以 NUL 結尾，輸出指標指向可寫入的本地儲存，建立範圍限於目前使用者。
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
        // SAFETY: 根鍵與以 NUL 結尾的子鍵路徑有效，key 指向可寫入的 handle 儲存位置。
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
/// 持有單一 Registry handle，離開作用域時關閉一次。
struct RegistryKey(HKEY);

#[cfg(windows)]
impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: handle 來自成功的開啟或建立呼叫，僅在此 RAII 守衛釋放時關閉一次。
        unsafe { RegCloseKey(self.0) };
    }
}
