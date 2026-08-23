use super::UserRuntime;
use crate::app_state::CommandResult;
use local_store::UserSettings;

impl UserRuntime {
    pub(crate) async fn user_settings(&self) -> CommandResult<UserSettings> {
        self.with_store(|store| store.user_settings().map_err(|error| error.to_string()))
            .await
    }

    pub(crate) async fn save_user_settings(&self, settings: UserSettings) -> CommandResult<()> {
        self.with_store(move |store| {
            store
                .save_user_settings(settings)
                .map_err(|error| error.to_string())
        })
        .await
    }
}
