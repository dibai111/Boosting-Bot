use super::StartMatchmakingInput;
use crate::{
    bot_runtime::{GameKind, GameMode},
    input_limits::{validate_length, MAX_BOT_ID_BYTES, MAX_LOG_PATH_BYTES},
};
use anyhow::{bail, Result};
use std::collections::HashMap;

/// 將 wire request 與已儲存帳號快照編譯成單次 matchmaking 可執行的不可變資料。
#[derive(Debug, Clone)]
pub(crate) struct MatchmakingPlan {
    mode: GameMode,
    bots: Vec<PlannedBot>,
    verify_presence: bool,
    verify_duel_pitch: bool,
    required_matches: usize,
    log_path: String,
}

#[derive(Debug, Clone)]
struct PlannedBot {
    id: String,
    username: String,
}

impl MatchmakingPlan {
    pub(crate) fn compile(
        mut input: StartMatchmakingInput,
        accounts: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self> {
        normalize_bot_ids(&mut input.bot_ids);
        validate_input(&input)?;

        let accounts = accounts.into_iter().collect::<HashMap<_, _>>();
        let mut bots = Vec::with_capacity(input.bot_ids.len());
        for id in input.bot_ids {
            let Some(username) = accounts.get(&id) else {
                bail!("one or more selected bots no longer exist");
            };
            bots.push(PlannedBot {
                id,
                username: username.clone(),
            });
        }
        if input.mode.kind() == GameKind::Bedwars
            && input.verify_presence
            && bots.iter().any(|bot| bot.username.trim().is_empty())
        {
            bail!("chat verification requires every selected bot username");
        }

        Ok(Self {
            mode: input.mode,
            bots,
            verify_presence: input.verify_presence,
            verify_duel_pitch: input.verify_duel_pitch,
            required_matches: input.required_matches,
            log_path: input.log_path,
        })
    }

    pub(crate) const fn mode(&self) -> GameMode {
        self.mode
    }

    pub(crate) fn bot_ids(&self) -> impl Iterator<Item = &str> {
        self.bots.iter().map(|bot| bot.id.as_str())
    }

    pub(crate) fn username(&self, bot_id: &str) -> Option<&str> {
        self.bots
            .iter()
            .find(|bot| bot.id == bot_id)
            .map(|bot| bot.username.as_str())
    }

    pub(crate) fn requires_presence_verification(&self) -> bool {
        self.mode.kind() == GameKind::Bedwars && self.verify_presence
    }

    pub(crate) fn requires_pitch_verification(&self) -> bool {
        self.mode.kind() == GameKind::Duels && self.verify_duel_pitch
    }

    pub(crate) const fn required_matches(&self) -> usize {
        self.required_matches
    }

    pub(crate) fn log_path(&self) -> &str {
        &self.log_path
    }
}

fn normalize_bot_ids(bot_ids: &mut Vec<String>) {
    for bot_id in bot_ids.iter_mut() {
        *bot_id = bot_id.trim().to_owned();
    }
    bot_ids.retain(|bot_id| !bot_id.is_empty());
    bot_ids.sort_unstable();
    bot_ids.dedup();
}

fn validate_input(input: &StartMatchmakingInput) -> Result<()> {
    if input.bot_ids.is_empty() {
        bail!("select at least one bot");
    }
    if input.bot_ids.len() > 32 {
        bail!("a session supports at most 32 bots");
    }
    if input.required_matches == 0 || input.required_matches > input.bot_ids.len() {
        bail!("minimum matches must be between 1 and the selected bot count");
    }
    if input.log_path.trim().is_empty() {
        bail!("select the player's latest.log file");
    }
    for bot_id in &input.bot_ids {
        validate_length(bot_id, "bot id", MAX_BOT_ID_BYTES)?;
    }
    validate_length(&input.log_path, "log path", MAX_LOG_PATH_BYTES)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(bot_ids: &[&str], required_matches: usize) -> StartMatchmakingInput {
        StartMatchmakingInput {
            mode: GameMode::BedwarsDoubles,
            bot_ids: bot_ids.iter().map(|id| (*id).to_owned()).collect(),
            verify_presence: true,
            verify_duel_pitch: true,
            required_matches,
            log_path: "latest.log".to_owned(),
        }
    }

    fn accounts(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(id, username)| ((*id).to_owned(), (*username).to_owned()))
            .collect()
    }

    #[test]
    fn normalizes_ids_and_snapshots_selected_accounts() {
        let plan = MatchmakingPlan::compile(
            input(&[" bot-b ", "", "bot-a", " bot-a "], 2),
            accounts(&[("bot-a", "Alpha"), ("bot-b", "Bravo")]),
        )
        .expect("valid plan");

        assert_eq!(plan.bot_ids().collect::<Vec<_>>(), ["bot-a", "bot-b"]);
        assert_eq!(plan.username("bot-b"), Some("Bravo"));
    }

    #[test]
    fn rejects_invalid_or_unknown_bot_selection() {
        assert!(MatchmakingPlan::compile(input(&["bot-a"], 2), accounts(&[])).is_err());
        assert!(MatchmakingPlan::compile(input(&["bot-a"], 1), accounts(&[])).is_err());

        let mut too_long = input(&["bot-a"], 1);
        too_long.log_path = "x".repeat(MAX_LOG_PATH_BYTES + 1);
        assert!(MatchmakingPlan::compile(too_long, accounts(&[("bot-a", "Alpha")])).is_err());
    }

    #[test]
    fn requires_a_username_only_for_bedwars_presence_verification() {
        assert!(
            MatchmakingPlan::compile(input(&["bot-a"], 1), accounts(&[("bot-a", "")])).is_err()
        );

        let mut duels = input(&["bot-a"], 1);
        duels.mode = GameMode::DuelsSumo;
        assert!(MatchmakingPlan::compile(duels, accounts(&[("bot-a", "")])).is_ok());
    }
}
