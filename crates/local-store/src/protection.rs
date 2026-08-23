use anyhow::{bail, Context, Result};

#[cfg(windows)]
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
    // SAFETY: The input blob points to an immutable slice that remains alive for this call;
    // output points to writable local storage, and all optional pointers are null by contract.
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
    // SAFETY: CryptProtectData allocates this buffer for the caller, and it is released exactly
    // once with the Windows allocator documented for DATA_BLOB results.
    unsafe { LocalFree(output.pbData.cast()) };
    protected
}

#[cfg(windows)]
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
    // SAFETY: The input blob points to an immutable slice that remains alive for this call;
    // output points to writable local storage, and optional description/entropy pointers are null.
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
    // SAFETY: CryptUnprotectData allocates this buffer for the caller, and it is released exactly
    // once with the Windows allocator documented for DATA_BLOB results.
    unsafe { LocalFree(output.pbData.cast()) };
    plaintext
}

#[cfg(windows)]
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
    // SAFETY: Windows returned a non-null buffer with cbData bytes for this blob.
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
