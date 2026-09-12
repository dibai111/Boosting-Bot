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

/** 先讀取本機設定再掛載主畫面；設定讀取失敗時交由畫面採用預設值。 */
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "../../shared-ui/ripple.css";
import { api } from "@botting/user-api";
import { watchUserCanvas } from "./lib/adapters/window";

const isMatchmakingOverlay =
  new URLSearchParams(window.location.search).get("overlay") === "matchmaking";
document.documentElement.dataset.userWindow = isMatchmakingOverlay ? "overlay" : "entry";
if (!isMatchmakingOverlay) watchUserCanvas();

void api
  .loadSettings()
  .catch(() => ({}))
  .then((settings) => {
    mount(App, { target: document.getElementById("app")!, props: { settings } });
  });
