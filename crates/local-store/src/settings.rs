use crate::{Store, UserSettings};
use anyhow::{bail, Result};

const MAX_SETTINGS: usize = 64;
const MAX_SETTING_KEY_BYTES: usize = 128;
const MAX_SETTING_VALUE_BYTES: usize = 16 * 1024;

impl Store {
    pub fn user_settings(&self) -> Result<UserSettings> {
        Ok(self.read_state()?.settings)
    }

    pub fn save_user_settings(&self, settings: UserSettings) -> Result<()> {
        if settings.len() > MAX_SETTINGS {
            bail!("too many local settings")
        }
        if settings.iter().any(|(key, value)| {
            key.is_empty()
                || key.len() > MAX_SETTING_KEY_BYTES
                || value.len() > MAX_SETTING_VALUE_BYTES
        }) {
            bail!("local setting is too large or has an empty key")
        }
        self.update_state(|state| {
            state.settings = settings;
            Ok(())
        })
    }
}
