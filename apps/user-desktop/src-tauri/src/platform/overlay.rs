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

//! 序列化所有浮動視窗操作，成功後讀取實際可見狀態，再發布同版本快照。
use crate::app_state::CommandResult;
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use tokio::sync::Mutex;

pub(crate) const MATCHMAKING_OVERLAY_LABEL: &str = "matchmaking-overlay";
const OVERLAY_VISIBILITY_EVENT: &str = "matchmaking-overlay-visibility";
const OVERLAY_DESIGN_WIDTH: u32 = 350;
const OVERLAY_DESIGN_BASE_HEIGHT: u32 = 179;
const OVERLAY_DESIGN_BOT_HEIGHT: u32 = 57;
const OVERLAY_MAX_VISIBLE_BOTS: usize = 5;
const REFERENCE_DPI_NUMERATOR: u32 = 5;
const REFERENCE_DPI_DENOMINATOR: u32 = 4;
const OVERLAY_RIGHT_MARGIN: i32 = 30;
const OVERLAY_TOP_MARGIN: i32 = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct OverlaySnapshot {
    pub revision: u64,
    pub visible: bool,
}

pub(crate) enum OverlayOperation {
    Show(usize),
    Hide,
    Toggle(usize),
    Read,
}

/// 統一原生視窗與測試替身的操作，讓控制器以同一流程確認實際可見狀態。
trait OverlayAdapter {
    fn show(&self, bot_count: usize) -> CommandResult<()>;
    fn hide(&self) -> CommandResult<()>;
    fn is_visible(&self) -> CommandResult<bool>;
    fn notify(&self, snapshot: OverlaySnapshot);
}

#[derive(Default)]
pub(crate) struct OverlayController {
    snapshot: Mutex<OverlaySnapshot>,
}
impl OverlayController {
    /// 完整操作持有鎖；只在原生操作成功之後發布實際狀態。
    pub(crate) async fn execute(
        &self,
        app: &AppHandle,
        operation: OverlayOperation,
    ) -> CommandResult<OverlaySnapshot> {
        self.apply(&TauriOverlay(app.clone()), operation).await
    }
    async fn apply(
        &self,
        adapter: &impl OverlayAdapter,
        operation: OverlayOperation,
    ) -> CommandResult<OverlaySnapshot> {
        let mut snapshot = self.snapshot.lock().await;
        let is_read = matches!(operation, OverlayOperation::Read);
        match operation {
            OverlayOperation::Show(count) => adapter.show(count)?,
            OverlayOperation::Hide => adapter.hide()?,
            OverlayOperation::Toggle(count) => {
                if adapter.is_visible()? {
                    adapter.hide()?;
                } else {
                    adapter.show(count)?;
                }
            }
            OverlayOperation::Read => {}
        }
        let visible = adapter.is_visible()?;
        // 查詢只在發現視窗狀態由外部改變時增加版本，避免重複通知桌面。
        if !is_read || visible != snapshot.visible {
            snapshot.revision = snapshot
                .revision
                .checked_add(1)
                .expect("overlay revision exhausted");
            snapshot.visible = visible;
            adapter.notify(*snapshot);
        }
        Ok(*snapshot)
    }
}

struct TauriOverlay(AppHandle);
impl OverlayAdapter for TauriOverlay {
    fn show(&self, bot_count: usize) -> CommandResult<()> {
        let overlay_size = overlay_size(bot_count);
        let overlay_position = overlay_position(&self.0, overlay_size);
        if let Some(window) = self.0.get_webview_window(MATCHMAKING_OVERLAY_LABEL) {
            window
                .set_size(overlay_size)
                .map_err(|error| error.to_string())?;
            window
                .set_position(overlay_position)
                .map_err(|error| error.to_string())?;
            window.show().map_err(|error| error.to_string())?;
            return Ok(());
        }

        let window = WebviewWindowBuilder::new(
            &self.0,
            MATCHMAKING_OVERLAY_LABEL,
            WebviewUrl::App("index.html?overlay=matchmaking".into()),
        )
        .title("Matchmaking")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?;
        window
            .set_size(overlay_size)
            .map_err(|error| error.to_string())?;
        window
            .set_position(overlay_position)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        Ok(())
    }
    fn hide(&self) -> CommandResult<()> {
        if let Some(window) = self.0.get_webview_window(MATCHMAKING_OVERLAY_LABEL) {
            window.hide().map_err(|error| error.to_string())?;
        }
        Ok(())
    }
    fn is_visible(&self) -> CommandResult<bool> {
        match self.0.get_webview_window(MATCHMAKING_OVERLAY_LABEL) {
            Some(window) => window.is_visible().map_err(|error| error.to_string()),
            None => Ok(false),
        }
    }
    fn notify(&self, snapshot: OverlaySnapshot) {
        let _ = self.0.emit(OVERLAY_VISIBILITY_EVENT, snapshot);
    }
}

fn overlay_position(app: &AppHandle, overlay_size: PhysicalSize<u32>) -> PhysicalPosition<i32> {
    let Ok(Some(monitor)) = app.primary_monitor() else {
        return PhysicalPosition::new(OVERLAY_RIGHT_MARGIN, OVERLAY_TOP_MARGIN);
    };
    let size = monitor.size();
    let position = monitor.position();
    let monitor_width = i32::try_from(size.width).unwrap_or(i32::MAX);
    let overlay_width = i32::try_from(overlay_size.width).unwrap_or(i32::MAX);
    let right = position.x.saturating_add(monitor_width);

    PhysicalPosition::new(
        right
            .saturating_sub(overlay_width)
            .saturating_sub(OVERLAY_RIGHT_MARGIN),
        position.y.saturating_add(OVERLAY_TOP_MARGIN),
    )
}

fn overlay_size(bot_count: usize) -> PhysicalSize<u32> {
    let visible_bots = bot_count.clamp(1, OVERLAY_MAX_VISIBLE_BOTS) as u32;
    let design_height = OVERLAY_DESIGN_BASE_HEIGHT + visible_bots * OVERLAY_DESIGN_BOT_HEIGHT;

    PhysicalSize::new(
        reference_pixels(OVERLAY_DESIGN_WIDTH),
        reference_pixels(design_height),
    )
}

fn reference_pixels(design_pixels: u32) -> u32 {
    (design_pixels
        .saturating_mul(REFERENCE_DPI_NUMERATOR)
        .saturating_add(REFERENCE_DPI_DENOMINATOR / 2))
        / REFERENCE_DPI_DENOMINATOR
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::{Cell, RefCell};
    #[derive(Default)]
    struct MemoryOverlay {
        visible: Cell<bool>,
        fail_show: Cell<bool>,
        snapshots: RefCell<Vec<OverlaySnapshot>>,
    }
    impl OverlayAdapter for MemoryOverlay {
        fn show(&self, _bot_count: usize) -> CommandResult<()> {
            if self.fail_show.get() {
                return Err("show failed".to_owned());
            }
            self.visible.set(true);
            Ok(())
        }
        fn hide(&self) -> CommandResult<()> {
            self.visible.set(false);
            Ok(())
        }
        fn is_visible(&self) -> CommandResult<bool> {
            Ok(self.visible.get())
        }
        fn notify(&self, snapshot: OverlaySnapshot) {
            assert_eq!(
                snapshot.visible,
                self.visible.get(),
                "notification must reflect the completed operation"
            );
            self.snapshots.borrow_mut().push(snapshot);
        }
    }
    #[tokio::test]
    async fn show_toggle_and_close_hide_publish_actual_state_in_order() {
        let controller = OverlayController::default();
        let adapter = MemoryOverlay::default();
        let shown = controller
            .apply(&adapter, OverlayOperation::Show(1))
            .await
            .unwrap();
        assert!(shown.visible);
        let hidden = controller
            .apply(&adapter, OverlayOperation::Toggle(1))
            .await
            .unwrap();
        assert!(!hidden.visible);
        assert!(hidden.revision > shown.revision);
        controller
            .apply(&adapter, OverlayOperation::Toggle(1))
            .await
            .unwrap();
        let closed = controller
            .apply(&adapter, OverlayOperation::Hide)
            .await
            .unwrap();
        assert!(!closed.visible);
        assert_eq!(closed.revision, 4);
        assert_eq!(
            controller
                .apply(&adapter, OverlayOperation::Read)
                .await
                .unwrap(),
            closed
        );
        assert_eq!(adapter.snapshots.borrow().len(), 4);
    }
    #[tokio::test]
    async fn failed_show_does_not_publish_success_or_consume_a_revision() {
        let controller = OverlayController::default();
        let adapter = MemoryOverlay::default();
        adapter.fail_show.set(true);
        assert!(controller
            .apply(&adapter, OverlayOperation::Show(1))
            .await
            .is_err());
        assert!(adapter.snapshots.borrow().is_empty());
        assert_eq!(
            controller
                .apply(&adapter, OverlayOperation::Read)
                .await
                .unwrap(),
            OverlaySnapshot::default()
        );
        adapter.fail_show.set(false);
        assert_eq!(
            controller
                .apply(&adapter, OverlayOperation::Show(1))
                .await
                .unwrap()
                .revision,
            1
        );
    }
    #[tokio::test]
    async fn reads_reconcile_an_external_visibility_change() {
        let controller = OverlayController::default();
        let adapter = MemoryOverlay::default();
        adapter.visible.set(true);
        let read = controller
            .apply(&adapter, OverlayOperation::Read)
            .await
            .unwrap();
        assert_eq!(
            read,
            OverlaySnapshot {
                revision: 1,
                visible: true
            }
        );
        assert_eq!(adapter.snapshots.borrow().as_slice(), &[read]);
    }

    #[test]
    fn one_bot_overlay_matches_the_reference_physical_dimensions() {
        assert_eq!(overlay_size(1), PhysicalSize::new(438, 295));
    }

    #[test]
    fn overlay_height_clamps_to_the_visible_bot_limit() {
        assert_eq!(overlay_size(0), overlay_size(1));
        assert_eq!(
            overlay_size(OVERLAY_MAX_VISIBLE_BOTS + 1),
            PhysicalSize::new(438, 580)
        );
    }
}
