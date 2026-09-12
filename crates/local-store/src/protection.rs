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

//! 封裝 Windows DPAPI 加解密，並以 LocalFree 釋放系統配置的輸出緩衝區。

use anyhow::{bail, Context, Result};

#[cfg(windows)]
/// 以目前 Windows 使用者的 DPAPI 保護資料。
/// @param data 待加密的完整狀態位元組。
/// @return DPAPI 加密資料；API 失敗時回傳錯誤。
pub(crate) fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use std::ptr::null;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB},
    };

    let data_length = u32::try_from(data.len()).context("local state is too large")?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: data_length,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: 輸入切片在呼叫期間保持有效；output 指向可寫入的本地結構，可選參數依 API 契約傳入空指標。
    let succeeded = unsafe {
        CryptProtectData(
            &input,
            null(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error()).context("protect local state with DPAPI");
    }

    let protected = copy_blob(&output).context("read DPAPI output");
    // SAFETY: 輸出由 CryptProtectData 配置，依 Windows 契約使用 LocalFree 恰好釋放一次。
    unsafe { LocalFree(output.pbData.cast()) };
    protected
}

#[cfg(windows)]
/// 解密目前使用者可存取的 DPAPI 資料。
/// @param data Registry 讀出的加密位元組。
/// @return 明文位元組；資料損壞或解密失敗時回傳錯誤。
pub(crate) fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };

    let data_length = u32::try_from(data.len()).context("protected local state is too large")?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: data_length,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: 輸入切片在呼叫期間保持有效；output 可寫入，描述與額外熵等可選參數傳入空指標。
    let succeeded = unsafe {
        CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error()).context("unprotect local state with DPAPI");
    }

    let plaintext = copy_blob(&output).context("read DPAPI plaintext");
    // SAFETY: 輸出由 CryptUnprotectData 配置，依 Windows 契約使用 LocalFree 恰好釋放一次。
    unsafe { LocalFree(output.pbData.cast()) };
    plaintext
}

#[cfg(windows)]
/// 檢查 Windows 輸出指標並複製資料，不接管其配置。
/// @param blob 仍有效且尚未 LocalFree 的 DPAPI 輸出。
/// @return 獨立位元組陣列；非空長度搭配空指標時回傳錯誤。
fn copy_blob(
    blob: &windows_sys::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB,
) -> Result<Vec<u8>> {
    use std::slice;

    if blob.cbData == 0 {
        return Ok(Vec::new());
    }
    if blob.pbData.is_null() {
        bail!("DPAPI returned a null data pointer")
    }
    let length = usize::try_from(blob.cbData).context("invalid DPAPI output length")?;
    // SAFETY: Windows 回傳的非空緩衝區包含 cbData 個位元組，並在複製完成前保持有效。
    Ok(unsafe { slice::from_raw_parts(blob.pbData, length) }.to_vec())
}

#[cfg(not(windows))]
pub(crate) fn protect(_data: &[u8]) -> Result<Vec<u8>> {
    bail!("encrypted local storage is only supported on Windows")
}

#[cfg(not(windows))]
pub(crate) fn unprotect(_data: &[u8]) -> Result<Vec<u8>> {
    bail!("encrypted local storage is only supported on Windows")
}
