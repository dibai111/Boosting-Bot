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

//! 封裝模式、執行識別及參與者；外部不能自行修改欄位或組合無效狀態。
use crate::{
    app_state::CommandResult,
    bot_runtime::{BotEvent, BotEventBus, RuntimeMode, RuntimeModeSnapshot},
};
use std::collections::HashSet;

enum WorkMode {
    Idle,
    Matching,
    Nick { run_id: u64, bots: HashSet<String> },
    Closed,
}

/// 啟停呼叫端持有 work 鎖涵蓋完整操作，狀態轉移只透過方法進行。
pub(super) struct WorkState {
    state: WorkMode,
    next_run_id: u64,
    accepted_nick_run: Option<u64>,
    revision: u64,
}
impl WorkState {
    pub fn new() -> Self {
        Self {
            state: WorkMode::Idle,
            next_run_id: 0,
            accepted_nick_run: None,
            revision: 0,
        }
    }
    pub fn mode(&self) -> RuntimeMode {
        match self.state {
            WorkMode::Idle | WorkMode::Closed => RuntimeMode::Idle,
            WorkMode::Matching => RuntimeMode::Matching,
            WorkMode::Nick { .. } => RuntimeMode::NickRoller,
        }
    }
    /// 命令回覆、初始化及事件都取用同一份快照。
    pub fn snapshot(&self) -> RuntimeModeSnapshot {
        RuntimeModeSnapshot {
            revision: self.revision,
            mode: self.mode(),
            nick_run_id: match &self.state {
                WorkMode::Nick { run_id, .. } => Some(*run_id),
                _ => None,
            },
        }
    }
    fn publish(&mut self, events: &BotEventBus) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("mode revision exhausted");
        events.publish(BotEvent::RuntimeMode {
            snapshot: self.snapshot(),
        });
    }
    fn ensure_idle(&self) -> CommandResult<()> {
        match self.state {
            WorkMode::Idle => Ok(()),
            WorkMode::Closed => Err("Botting is shutting down".to_owned()),
            WorkMode::Matching => {
                Err("Matching is currently active; stop it before starting Nick Roller".to_owned())
            }
            WorkMode::Nick { .. } => {
                Err("Nick Roller is currently active; stop it before starting Matching".to_owned())
            }
        }
    }
    /// 宣告配對啟動，並使上次 Nick 的剩餘事件失效。
    pub fn start_matching(&mut self, events: &BotEventBus) -> CommandResult<()> {
        self.ensure_idle()?;
        self.accepted_nick_run = None;
        self.state = WorkMode::Matching;
        self.publish(events);
        Ok(())
    }
    /// 一次建立 Nick 執行身分與非空參與者，避免外部逐欄宣告。
    pub fn start_nick(
        &mut self,
        bots: impl IntoIterator<Item = String>,
        events: &BotEventBus,
    ) -> CommandResult<u64> {
        self.ensure_idle()?;
        let bots: HashSet<_> = bots.into_iter().collect();
        if bots.is_empty() {
            return Err("select at least one bot".to_owned());
        }
        self.next_run_id = self
            .next_run_id
            .checked_add(1)
            .expect("Nick run id exhausted");
        let run_id = self.next_run_id;
        self.accepted_nick_run = Some(run_id);
        self.state = WorkMode::Nick { run_id, bots };
        self.publish(events);
        Ok(run_id)
    }
    pub fn finish_matching(&mut self, events: &BotEventBus) {
        if matches!(self.state, WorkMode::Matching) {
            self.become_idle(events);
        }
    }
    pub fn finish_nick(&mut self, expected_run: u64, events: &BotEventBus) {
        if matches!(&self.state, WorkMode::Nick { run_id, .. } if *run_id == expected_run) {
            self.become_idle(events);
        }
    }
    fn become_idle(&mut self, events: &BotEventBus) {
        self.state = WorkMode::Idle;
        self.publish(events);
    }
    /// 啟動發送失敗或 terminal 事件都以相同方式完成參與者；最後一個結束時釋放模式。
    pub fn complete_nick_bot(&mut self, expected_run: u64, bot_id: &str, events: &BotEventBus) {
        if let WorkMode::Nick { run_id, bots } = &mut self.state {
            if *run_id != expected_run {
                return;
            }
            bots.remove(bot_id);
            if bots.is_empty() {
                self.become_idle(events);
            }
        }
    }
    /// 停止操作取得不可變執行快照；外部不能清空或加入參與者。
    pub fn nick_participants(&self) -> Option<(u64, Vec<String>)> {
        match &self.state {
            WorkMode::Nick { run_id, bots } => Some((*run_id, bots.iter().cloned().collect())),
            _ => None,
        }
    }
    pub fn nick_run_for(&self, bot_id: &str) -> CommandResult<u64> {
        match &self.state {
            WorkMode::Nick { run_id, bots } if bots.contains(bot_id) => Ok(*run_id),
            _ => Err("Nick Roller is not active".to_owned()),
        }
    }
    /// 關閉先封鎖所有新工作，再回傳需停止的 Nick 執行。
    pub fn begin_shutdown(&mut self, events: &BotEventBus) -> Option<(u64, Vec<String>)> {
        let nick = self.nick_participants();
        self.state = WorkMode::Closed;
        self.accepted_nick_run = None;
        self.publish(events);
        nick
    }
    /// terminal state 之後仍可接收同次結果，直到下一次工作開始。
    pub fn accepts_nick(&self, run_id: u64) -> bool {
        self.accepted_nick_run == Some(run_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queries_and_events_share_one_monotonic_mode_revision() {
        let events = BotEventBus::new();
        let mut receiver = events.take_business_events();
        let mut work = WorkState::new();
        assert_eq!(work.snapshot().revision, 0);
        let run_id = work.start_nick(["a".to_owned()], &events).unwrap();
        let started = work.snapshot();
        assert_eq!(started.nick_run_id, Some(run_id));
        let BotEvent::RuntimeMode { snapshot } = receiver.try_recv().unwrap() else {
            panic!("expected mode event")
        };
        assert_eq!(snapshot.revision, started.revision);
        assert_eq!(snapshot.mode, started.mode);
        assert_eq!(snapshot.nick_run_id, started.nick_run_id);
        work.complete_nick_bot(run_id, "a", &events);
        let finished = work.snapshot();
        assert!(finished.revision > started.revision);
        assert_eq!(finished.mode, RuntimeMode::Idle);
        assert_eq!(finished.nick_run_id, None);
        assert!(work.start_nick([], &events).is_err());
        assert_eq!(work.snapshot().revision, finished.revision);
        work.start_matching(&events).unwrap();
        assert!(work.snapshot().revision > finished.revision);
        let encoded = serde_json::to_value(BotEvent::RuntimeMode { snapshot: started }).unwrap();
        assert_eq!(encoded["type"], "runtime_mode");
        assert_eq!(encoded["revision"], started.revision);
        assert_eq!(encoded["mode"], "nick_roller");
    }

    #[test]
    fn conflicts_and_stale_completions_cannot_release_a_new_run() {
        let events = BotEventBus::new();
        let mut work = WorkState::new();
        let first = work
            .start_nick(["a".to_owned(), "b".to_owned()], &events)
            .unwrap();
        assert!(work.start_matching(&events).is_err());
        work.complete_nick_bot(first, "a", &events);
        assert_eq!(work.mode(), RuntimeMode::NickRoller);
        work.complete_nick_bot(first, "b", &events);
        assert_eq!(work.mode(), RuntimeMode::Idle);
        assert!(work.accepts_nick(first));
        let second = work.start_nick(["a".to_owned()], &events).unwrap();
        work.complete_nick_bot(first, "a", &events);
        work.finish_nick(first, &events);
        assert_eq!(work.nick_run_for("a").unwrap(), second);
        assert!(!work.accepts_nick(first));
        work.finish_nick(second, &events);
        work.start_matching(&events).unwrap();
        assert!(!work.accepts_nick(second));
        work.finish_matching(&events);
    }
    #[test]
    fn shutdown_is_terminal_and_empty_runs_cannot_start() {
        let events = BotEventBus::new();
        let mut work = WorkState::new();
        assert!(work.start_nick([], &events).is_err());
        let run = work.start_nick(["a".to_owned()], &events).unwrap();
        assert_eq!(
            work.begin_shutdown(&events),
            Some((run, vec!["a".to_owned()]))
        );
        work.finish_nick(run, &events);
        work.finish_matching(&events);
        assert!(work.start_matching(&events).is_err());
        assert!(work.start_nick(["a".to_owned()], &events).is_err());
        assert!(!work.accepts_nick(run));
    }
}
