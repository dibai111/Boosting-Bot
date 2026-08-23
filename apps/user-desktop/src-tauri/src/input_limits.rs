use anyhow::{bail, Result};

pub(crate) const MAX_BOT_ID_BYTES: usize = 128;
pub(crate) const MAX_CREDENTIAL_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_LOG_PATH_BYTES: usize = 4 * 1024;
pub(crate) const MAX_SERVER_ADDRESS_BYTES: usize = 255;
pub(crate) const MAX_USERNAME_BYTES: usize = 64;

pub(crate) fn validate_length(value: &str, field: &str, max_bytes: usize) -> Result<()> {
    if value.len() > max_bytes {
        bail!("{field} exceeds {max_bytes} bytes");
    }
    Ok(())
}
