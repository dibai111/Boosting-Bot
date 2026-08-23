use super::UserRuntime;
use crate::{
    app_state::CommandResult,
    matchmaking::{MatchmakingPlan, MatchmakingSnapshot, StartMatchmakingInput},
};

impl UserRuntime {
    pub(crate) async fn start_matchmaking(
        &self,
        input: StartMatchmakingInput,
    ) -> CommandResult<MatchmakingSnapshot> {
        let accounts = self
            .with_store(|store| store.list_accounts().map_err(|error| error.to_string()))
            .await?;
        let plan = MatchmakingPlan::compile(
            input,
            accounts
                .iter()
                .map(|account| (account.id.clone(), account.username.clone())),
        )
        .map_err(|error| error.to_string())?;
        self.matchmaking
            .start(plan)
            .await
            .map_err(|error| format!("start matchmaking: {error:#}"))
    }

    pub(crate) async fn stop_matchmaking(&self) -> MatchmakingSnapshot {
        self.matchmaking.stop().await;
        self.matchmaking.snapshot().await
    }

    pub(crate) async fn matchmaking_snapshot(&self) -> MatchmakingSnapshot {
        self.matchmaking.snapshot().await
    }
}
