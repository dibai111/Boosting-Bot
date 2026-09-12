<!--
SPDX-License-Identifier: AGPL-3.0-only
Copyright (C) 2026 baibai and Botting contributors

Botting is free software: you can redistribute it and/or modify it under
the GNU Affero General Public License version 3, as published by the
Free Software Foundation. This program comes WITHOUT ANY WARRANTY;
without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the LICENSE file for the complete terms.
Copyleft: covered modifications must retain these license obligations.
https://www.gnu.org/licenses/agpl-3.0.html
-->

<script lang="ts">
  // 管理 Nick 表單、候選決定及音效；事件投影交由獨立的 view builder 處理。
  import { onMount } from "svelte";
  import {
    Activity,
    Box,
    Check,
    Info,
    Play,
    ShieldCheck,
    Square,
    Volume2,
    VolumeX,
    X,
  } from "@lucide/svelte";
  import { api } from "@botting/user-api";
  import { createNickBotViewBuilder, type NickBotView } from "../../features/nick-roller-view";
  import {
    defaultNickRollerConfig,
    parseNickRollerConfig,
  } from "../../features/nick-roller-config";
  import type { MessageKey } from "../../i18n";
  import type {
    Account,
    BotEvent,
    BotPhase,
    NickRollerConfig,
    NickRollerEngineConfig,
    NickRollerPhase,
    RuntimeMode,
  } from "../../models/types";

  export let accounts: Account[] = [];
  export let phases: Record<string, BotPhase> = {};
  export let selectedBotIds = new Set<string>();
  export let activeMode: RuntimeMode = "idle";
  export let events: BotEvent[] = [];
  export let settings: Record<string, string> = {};
  export let t: (key: MessageKey) => string;
  export let settingsOnly = false;
  export let onToggleBot: (botId: string) => void = () => {};
  export let savePreference: (key: string, value: string) => void = () => {};
  export let showToast: (message: string, kind: "success" | "error") => void = () => {};

  const configStorageKey = "botting-nick-roller-config";
  const maxActivityEntries = 80;
  let config = defaultNickRollerConfig();
  let loadedConfigValue = "";
  let saving = false;
  let starting = false;
  let now = Date.now();
  let audioContext: AudioContext | null = null;
  let lastSoundCandidateKey: string | null | undefined;
  const buildBotView = createNickBotViewBuilder();

  $: if (settings[configStorageKey] !== loadedConfigValue) {
    loadedConfigValue = settings[configStorageKey] ?? "";
    config = parseNickRollerConfig(loadedConfigValue);
  }
  $: onlineBots = accounts.filter((account) => phases[account.id] === "online");
  $: selectedOnlineBots = onlineBots.filter((account) => selectedBotIds.has(account.id));
  $: botViews = accounts
    .filter((account) => selectedBotIds.has(account.id))
    .map((account) => buildBotView(account, events, activeMode));
  $: activeBotViews = botViews.filter(
    (bot) =>
      bot.phase !== "idle" &&
      bot.phase !== "finished" &&
      bot.phase !== "failed" &&
      bot.phase !== "stopped",
  );
  $: currentCandidates = botViews.filter(
    (bot) => bot.candidateId !== null && bot.candidate.length > 0,
  );
  $: activityEvents = events
    .filter(
      (event) =>
        event.type === "nick_roller_state" ||
        event.type === "nick_candidate" ||
        event.type === "nick_verification" ||
        event.type === "nick_attention",
    )
    .filter((event) => config.show_rejected || event.type !== "nick_candidate" || event.accepted)
    .slice(-maxActivityEntries)
    .reverse();
  $: if (activeMode === "nick_roller") {
    now = Date.now();
  }

  $: {
    // 將事件陣列明確列為依賴，讓新候選結果觸發 Svelte 更新。
    const candidateKey = latestAcceptedCandidateKey(events);
    if (
      lastSoundCandidateKey !== undefined &&
      candidateKey &&
      candidateKey !== lastSoundCandidateKey
    ) {
      playSuccessTone();
    }
    if (lastSoundCandidateKey !== undefined) lastSoundCandidateKey = candidateKey;
  }

  onMount(() => {
    lastSoundCandidateKey = latestAcceptedCandidateKey(events);
    const timer = window.setInterval(() => {
      now = Date.now();
    }, 250);
    return () => {
      window.clearInterval(timer);
      void audioContext?.close().catch(() => undefined);
      audioContext = null;
    };
  });

  function phaseLabel(phase: NickRollerPhase): string {
    const labels: Record<NickRollerPhase, string> = {
      idle: t("offline"),
      preparing: t("connecting"),
      rolling: t("nickRoller"),
      awaiting_decision: t("nickRollerWaitingDecision"),
      verifying: t("nickRollerVerified"),
      finished: t("nickRollerVerified"),
      failed: t("error"),
      stopped: t("stopping"),
    };
    return labels[phase];
  }

  function updateRule<K extends keyof NickRollerConfig["rules"]>(
    key: K,
    value: NickRollerConfig["rules"][K],
  ): void {
    config = { ...config, rules: { ...config.rules, [key]: value } };
  }

  function updateLength(key: "exact_length" | "min_length" | "max_length", event: Event): void {
    const value = (event.currentTarget as HTMLInputElement).value;
    updateRule(key, value === "" ? null : Math.max(1, Math.min(16, Number(value))));
  }

  function updateNumber(key: "book_timeout_ms" | "next_roll_delay_ms", event: Event): void {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    config = {
      ...config,
      [key]: Number.isFinite(value) ? Math.max(0, Math.min(60_000, Math.round(value))) : 0,
    };
  }

  function textToList(value: string): string[] {
    return value
      .split(/\r?\n|,/)
      .map((item) => item.trim())
      .filter(Boolean)
      .slice(0, 256);
  }

  function listToText(value: string[]): string {
    return value.join("\n");
  }

  /**
   * 將目前規則與音效偏好序列化交給設定儲存回呼。
   * @return 提交設定的 Promise；實際持久化由父層負責。
   */
  async function saveConfig(): Promise<void> {
    saving = true;
    try {
      savePreference(configStorageKey, JSON.stringify(config));
      showToast(t("nickRollerSave"), "success");
    } finally {
      saving = false;
    }
  }

  /**
   * 驗證模式、線上 Bot 與長度規則後，送出後端篩選設定。
   * @return 啟動流程的 Promise；錯誤透過通知顯示。
   */
  async function startRoller(): Promise<void> {
    if (activeMode !== "idle") {
      showToast(t("nickRollerRunningLock"), "error");
      return;
    }
    if (!selectedOnlineBots.length) {
      showToast(t("nickRollerNoOnlineBots"), "error");
      return;
    }
    if (
      config.rules.exact_length !== null &&
      (config.rules.min_length !== null || config.rules.max_length !== null)
    ) {
      showToast(t("nickRollerExactLength"), "error");
      return;
    }
    starting = true;
    try {
      const engineConfig: NickRollerEngineConfig = {
        book_timeout_ms: config.book_timeout_ms,
        next_roll_delay_ms: config.next_roll_delay_ms,
        stop_after_found: config.stop_after_found,
        rules: config.rules,
      };
      await api.startNickRoller({
        bot_ids: selectedOnlineBots.map((account) => account.id),
        config: engineConfig,
      });
      showToast(t("nickRollerStarted"), "success");
    } catch (error) {
      const message = String(error);
      showToast(message.includes("mc.hypixel.net") ? t("nickRollerHypixelOnly") : message, "error");
    } finally {
      starting = false;
    }
  }

  /**
   * 停止本次 Nick 篩選並顯示操作結果。
   * @return 停止流程的 Promise。
   */
  async function stopRoller(): Promise<void> {
    try {
      await api.stopNickRoller();
      showToast(t("nickRollerStopped"), "success");
    } catch (error) {
      showToast(String(error), "error");
    }
  }

  /**
   * 對畫面仍持有的候選送出決策。
   * @param bot 包含候選編號的 Bot 畫面狀態。
   * @param take true 接受，false 跳過。
   * @return 決策發送流程的 Promise。
   */
  async function decide(bot: NickBotView, take: boolean): Promise<void> {
    if (bot.candidateId === null) return;
    try {
      await api.answerNickDecision(bot.account.id, bot.candidateId, take);
    } catch (error) {
      showToast(String(error), "error");
    }
  }

  function toggleSound(): void {
    config = {
      ...config,
      sound: {
        ...config.sound,
        enabled: !config.sound.enabled,
      },
    };
    void saveConfig();
  }

  function updateVolume(event: Event): void {
    const volume = Math.max(
      0,
      Math.min(100, Number((event.currentTarget as HTMLInputElement).value)),
    );
    config = {
      ...config,
      sound: { ...config.sound, volume, enabled: volume > 0 },
    };
    void saveConfig();
  }

  /**
   * 以目前音量播放短提示音，按需建立 AudioContext。
   * @return 無回傳值；音訊不可用時不阻擋候選操作。
   */
  function playSuccessTone(): void {
    if (!config.sound.enabled || config.sound.volume <= 0) return;
    try {
      audioContext ??= new AudioContext();
      void audioContext.resume().catch(() => undefined);
      const oscillator = audioContext.createOscillator();
      const gain = audioContext.createGain();
      const start = audioContext.currentTime;
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.exponentialRampToValueAtTime(config.sound.volume / 500, start + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.28);
      oscillator.frequency.setValueAtTime(659.25, start);
      oscillator.frequency.exponentialRampToValueAtTime(987.77, start + 0.18);
      oscillator.connect(gain).connect(audioContext.destination);
      oscillator.start(start);
      oscillator.stop(start + 0.3);
    } catch {
      audioContext = null;
    }
  }

  function latestAcceptedCandidateKey(botEvents: BotEvent[]): string | null {
    // 從尾端查找即可，毋須複製並反轉整個事件陣列。
    for (let index = botEvents.length - 1; index >= 0; index -= 1) {
      const event = botEvents[index];
      if (event.type === "nick_candidate" && event.accepted)
        return `${event.bot_id}:${event.candidate_id}`;
    }
    return null;
  }

  function eventTitle(event: BotEvent): string {
    if (event.type === "nick_candidate")
      return event.accepted ? t("nickRollerCandidateReceived") : t("nickRollerRejected");
    if (event.type === "nick_verification")
      return event.success ? t("nickRollerVerified") : t("nickRollerVerificationFailed");
    if (event.type === "nick_attention") return t("nickRollerInvalidBooks");
    if (event.type === "nick_roller_state") return event.message ?? phaseLabel(event.phase);
    return t("nickRollerActivity");
  }

  function eventTone(event: BotEvent): string {
    if (event.type === "nick_attention" || (event.type === "nick_verification" && !event.success))
      return "danger";
    if (event.type === "nick_candidate" && !event.accepted) return "muted";
    if (event.type === "nick_verification" && event.success) return "success";
    return "accent";
  }

  function formatDeadline(deadline: number | null): string {
    if (!deadline) return "";
    return `${Math.max(0, Math.ceil((deadline - now) / 1000))}s`;
  }
</script>

<section class="nick-panel">
  {#if !settingsOnly}
    <section class="nick-card nick-bot-list">
      <div class="nick-bot-list-header">
        <div class="nick-section-title">
          <span class="nick-section-icon"><Box size={16} /></span>
          <span class="nick-section-title-copy"
            ><strong>{t("nickRollerSelectBots")}</strong><small
              >{selectedOnlineBots.length}/{onlineBots.length} {t("online")}</small
            ></span
          >
        </div>
        <div class="nick-bot-actions">
          <button
            class="nick-action-button"
            disabled={starting || activeMode !== "idle" || !selectedOnlineBots.length}
            title={activeMode === "matching" ? t("nickRollerMatchingActive") : undefined}
            on:click={startRoller}
          >
            <Play size={15} />{t("nickRollerStart")}
          </button>
          {#if activeMode === "nick_roller"}
            <button class="nick-action-button danger" on:click={stopRoller}
              ><Square size={15} />{t("nickRollerStop")}</button
            >
          {/if}
        </div>
      </div>
      <div class="nick-bot-grid">
        {#each accounts as account (account.id)}
          <button
            type="button"
            class="nick-bot-card"
            class:selected={selectedBotIds.has(account.id)}
            class:online={phases[account.id] === "online"}
            disabled={activeMode !== "idle" || phases[account.id] !== "online"}
            on:click={() => onToggleBot(account.id)}
          >
            <span class="nick-bot-avatar"><Box size={20} /></span>
            <span class="nick-bot-copy"
              ><strong>{account.username}</strong><small
                >{phases[account.id] === "online" ? t("online") : t("offline")}</small
              ></span
            >
            {#if selectedBotIds.has(account.id)}<Check size={16} class="nick-selected-mark" />{/if}
          </button>
        {:else}
          <p class="nick-empty">{t("noAccounts")}</p>
        {/each}
      </div>
    </section>
  {/if}

  {#if settingsOnly}
    <div class="nick-settings-layout">
      <article class="nick-card nick-rules-card">
        <header>
          <div>
            <h3>{t("nickRollerRules")}</h3>
            <p>{t("nickRollerSettingsStored")}</p>
          </div>
          <button class="nick-save-button" disabled={saving} on:click={saveConfig}
            ><Check size={14} />{t("nickRollerSave")}</button
          >
        </header>
        <div class="nick-field-grid">
          <label
            ><span>{t("nickRollerMaxLength")}</span><input
              type="number"
              min="1"
              max="16"
              value={config.rules.max_length ?? ""}
              disabled={activeMode === "nick_roller"}
              on:input={(event) => updateLength("max_length", event)}
            /></label
          >
          <label
            ><span>{t("nickRollerMinLength")}</span><input
              type="number"
              min="1"
              max="16"
              value={config.rules.min_length ?? ""}
              disabled={activeMode === "nick_roller"}
              on:input={(event) => updateLength("min_length", event)}
            /></label
          >
          <label
            ><span>{t("nickRollerExactLength")}</span><input
              type="number"
              min="1"
              max="16"
              placeholder="—"
              value={config.rules.exact_length ?? ""}
              disabled={activeMode === "nick_roller"}
              on:input={(event) => updateLength("exact_length", event)}
            /></label
          >
          <label
            ><span>{t("nickRollerBookTimeout")}</span><input
              type="number"
              min="500"
              max="30000"
              step="100"
              value={config.book_timeout_ms}
              disabled={activeMode === "nick_roller"}
              on:input={(event) => updateNumber("book_timeout_ms", event)}
            /></label
          >
          <label
            ><span>{t("nickRollerNextDelay")}</span><input
              type="number"
              min="0"
              max="60000"
              step="100"
              value={config.next_roll_delay_ms}
              disabled={activeMode === "nick_roller"}
              on:input={(event) => updateNumber("next_roll_delay_ms", event)}
            /></label
          >
        </div>
        <div class="nick-toggle-grid">
          {#each [["allow_numbers", "nickRollerAllowNumbers"], ["allow_underscore", "nickRollerAllowUnderscore"], ["legacy_priority", "nickRollerLegacyPriority"], ["stop_after_found", "nickRollerAutoTake"], ["case_sensitive", "nickRollerCaseSensitive"], ["show_rejected", "nickRollerShowRejected"]] as [key, label]}
            <label class="nick-toggle-row">
              <span>{t(label as MessageKey)}</span>
              <button
                type="button"
                class="nick-switch"
                class:on={key === "stop_after_found"
                  ? config.stop_after_found
                  : key === "show_rejected"
                    ? config.show_rejected
                    : Boolean(config.rules[key as keyof typeof config.rules])}
                role="switch"
                aria-label={t(label as MessageKey)}
                aria-checked={key === "stop_after_found"
                  ? config.stop_after_found
                  : key === "show_rejected"
                    ? config.show_rejected
                    : Boolean(config.rules[key as keyof typeof config.rules])}
                disabled={activeMode === "nick_roller"}
                on:click={() => {
                  if (key === "show_rejected")
                    config = { ...config, show_rejected: !config.show_rejected };
                  else if (key === "stop_after_found")
                    config = { ...config, stop_after_found: !config.stop_after_found };
                  else
                    updateRule(
                      key as keyof typeof config.rules,
                      !Boolean(config.rules[key as keyof typeof config.rules]) as never,
                    );
                }}><span></span></button
              >
            </label>
          {/each}
        </div>
        <div class="nick-list-grid">
          <label
            ><span>{t("nickRollerAlwaysAccept")}</span><textarea
              rows="2"
              placeholder={t("nickRollerOnePerLine")}
              value={listToText(config.rules.allow_list)}
              disabled={activeMode === "nick_roller"}
              on:input={(event) =>
                updateRule(
                  "allow_list",
                  textToList((event.currentTarget as HTMLTextAreaElement).value),
                )}></textarea></label
          >
          <label
            ><span
              >{t("nickRollerStartsWith")}
              <button
                type="button"
                class="nick-priority"
                class:on={config.rules.starts_with_priority}
                disabled={activeMode === "nick_roller"}
                on:click={() =>
                  updateRule("starts_with_priority", !config.rules.starts_with_priority)}
                >{t("nickRollerPriority")}</button
              ></span
            ><textarea
              rows="2"
              placeholder={t("nickRollerOnePerLine")}
              value={listToText(config.rules.starts_with)}
              disabled={activeMode === "nick_roller"}
              on:input={(event) =>
                updateRule(
                  "starts_with",
                  textToList((event.currentTarget as HTMLTextAreaElement).value),
                )}></textarea></label
          >
          <label
            ><span
              >{t("nickRollerEndsWith")}
              <button
                type="button"
                class="nick-priority"
                class:on={config.rules.ends_with_priority}
                disabled={activeMode === "nick_roller"}
                on:click={() => updateRule("ends_with_priority", !config.rules.ends_with_priority)}
                >{t("nickRollerPriority")}</button
              ></span
            ><textarea
              rows="2"
              placeholder={t("nickRollerOnePerLine")}
              value={listToText(config.rules.ends_with)}
              disabled={activeMode === "nick_roller"}
              on:input={(event) =>
                updateRule(
                  "ends_with",
                  textToList((event.currentTarget as HTMLTextAreaElement).value),
                )}></textarea></label
          >
          <label
            ><span
              >{t("nickRollerContains")}
              <button
                type="button"
                class="nick-priority"
                class:on={config.rules.contains_priority}
                disabled={activeMode === "nick_roller"}
                on:click={() => updateRule("contains_priority", !config.rules.contains_priority)}
                >{t("nickRollerPriority")}</button
              ></span
            ><textarea
              rows="2"
              placeholder={t("nickRollerOnePerLine")}
              value={listToText(config.rules.contains)}
              disabled={activeMode === "nick_roller"}
              on:input={(event) =>
                updateRule(
                  "contains",
                  textToList((event.currentTarget as HTMLTextAreaElement).value),
                )}></textarea><span class="nick-segmented"
              ><button
                type="button"
                class:active={config.rules.contains_match_mode === "any"}
                on:click={() => updateRule("contains_match_mode", "any")}
                >{t("nickRollerAny")}</button
              ><button
                type="button"
                class:active={config.rules.contains_match_mode === "all"}
                on:click={() => updateRule("contains_match_mode", "all")}
                >{t("nickRollerAll")}</button
              ></span
            ></label
          >
        </div>
      </article>
    </div>
  {:else}
    <div class="nick-main-grid nick-operation-grid">
      <aside class="nick-side-column">
        <article class="nick-card nick-candidate-card">
          <header>
            <h3>{t("nickRollerCurrentCandidate")}</h3>
            <span class="nick-live-mark"><span></span>{activeBotViews.length}</span>
          </header>
          {#if currentCandidates.length}
            <div class="nick-candidate-list">
              {#each currentCandidates as candidate (candidate.account.id)}
                <div class="nick-candidate-item">
                  <small>{candidate.account.username}</small>
                  <div class="nick-candidate-name">{candidate.candidate}</div>
                  <p class="nick-candidate-reason">{candidate.reasons.join("; ")}</p>
                  <p class="nick-decision-countdown">
                    {t("nickRollerAutoSkip")}
                    {formatDeadline(candidate.decisionDeadline)}
                  </p>
                  <div class="nick-candidate-actions">
                    <button class="nick-take" on:click={() => decide(candidate, true)}
                      ><Check size={15} />{t("nickRollerTake")}</button
                    ><button class="nick-skip" on:click={() => decide(candidate, false)}
                      ><X size={15} />{t("nickRollerSkip")}</button
                    >
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <div class="nick-empty-candidate">
              <Box size={26} /><span>{t("nickRollerNoCandidate")}</span>
            </div>
          {/if}
          <div class="nick-stats">
            <span
              ><strong>{botViews.reduce((total, bot) => total + bot.processed, 0)}</strong>{t(
                "nickRollerProcessed",
              )}</span
            ><span
              ><strong class="success-text"
                >{botViews.reduce((total, bot) => total + bot.matched, 0)}</strong
              >{t("nickRollerMatched")}</span
            ><span
              ><strong class="danger-text"
                >{botViews.reduce((total, bot) => total + bot.rejected, 0)}</strong
              >{t("nickRollerRejected")}</span
            >
          </div>
        </article>

        <article class="nick-card nick-sound-card">
          <header>
            <h3>{t("nickRollerSound")}</h3>
            <Volume2 size={16} />
          </header>
          <div class="nick-sound-row">
            <span>{t("nickRollerSound")}</span><button
              type="button"
              class="nick-icon-toggle"
              on:click={toggleSound}
              aria-label={t("nickRollerSound")}
            >
              {#if config.sound.enabled}<Volume2 size={16} />{:else}<VolumeX size={16} />{/if}
            </button>
          </div>
          <label class="nick-volume"
            ><span>{t("nickRollerVolume")} {config.sound.volume}%</span><input
              type="range"
              min="0"
              max="100"
              value={config.sound.volume}
              on:input={updateVolume}
            /></label
          >
          <p class="nick-secure-note"><ShieldCheck size={13} />{t("nickRollerSettingsStored")}</p>
        </article>
      </aside>
    </div>
  {/if}

  {#if !settingsOnly}
    {#if botViews.some((bot) => bot.verification || bot.attention)}
      <div class="nick-feedback-grid">
        {#each botViews as bot (bot.account.id)}
          {#if bot.verification}
            <article
              class="nick-feedback"
              class:success={bot.verification.success}
              class:failure={!bot.verification.success}
            >
              {#if bot.verification.success}<Check size={18} />{:else}<X size={18} />{/if}
              <div>
                <strong
                  >{bot.verification.success
                    ? t("nickRollerVerified")
                    : t("nickRollerVerificationFailed")}</strong
                ><span
                  >{bot.verification.success
                    ? `${bot.verification.expected_nick} · ${t("online")}`
                    : bot.verification.reason}</span
                >
              </div>
            </article>
          {/if}
          {#if bot.attention}
            <article class="nick-feedback warning">
              <Info size={18} />
              <div>
                <strong>{t("nickRollerInvalidBooks")}</strong><span
                  >{t("nickRollerMvpHint")}
                  {t("nickRollerLanguageHint")}
                  {t("nickRollerDailyHint")}</span
                >
              </div>
            </article>
          {/if}
        {/each}
      </div>
    {/if}

    <article class="nick-card nick-activity-card">
      <header>
        <div>
          <h3>{t("nickRollerActivity")}</h3>
          <p>{activeMode === "nick_roller" ? t("nickRollerActive") : t("nickRollerStopped")}</p>
        </div>
        <Activity size={17} />
      </header>
      <div class="nick-activity-list" aria-live="polite">
        {#each activityEvents as event, index (index)}
          <div class="nick-activity-row {eventTone(event)}">
            <span class="nick-activity-icon"
              >{#if eventTone(event) === "success"}<Check
                  size={13}
                />{:else if eventTone(event) === "danger"}<X size={13} />{:else}<Activity
                  size={13}
                />{/if}</span
            >
            <span>{eventTitle(event)}</span>
            {#if "bot_id" in event}<small
                >{accounts.find((account) => account.id === event.bot_id)?.username ??
                  t("bot")}</small
              >{/if}
          </div>
        {:else}
          <div class="nick-empty-activity"><Activity size={16} />{t("noSessionEvents")}</div>
        {/each}
      </div>
    </article>
  {/if}
</section>

<style>
  .nick-panel {
    display: grid;
    gap: 16px;
    padding-bottom: 28px;
  }
  .nick-card header p {
    margin: 4px 0 0;
    color: #92979c;
    font-size: 13px;
  }
  .nick-action-button,
  .nick-save-button,
  .nick-candidate-actions button {
    min-height: 36px;
    border: 1px solid #dce0e4;
    border-radius: 10px;
    color: #353a40;
    background: #fff;
    font-size: 13px;
    font-weight: 650;
  }
  .nick-action-button:first-of-type,
  .nick-save-button,
  .nick-take {
    color: #fff;
    border-color: #389df6;
    background: #389df6;
  }
  .nick-action-button.danger {
    color: #fff;
    border-color: #dc6259;
    background: #dc6259;
  }
  .nick-main-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.55fr) minmax(280px, 0.75fr);
    gap: 16px;
    align-items: start;
  }
  .nick-operation-grid {
    grid-template-columns: 1fr;
  }
  .nick-operation-grid .nick-side-column {
    grid-template-columns: minmax(0, 1.55fr) minmax(280px, 0.75fr);
  }
  .nick-settings-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }
  .nick-side-column {
    display: grid;
    gap: 16px;
  }
  .nick-card {
    min-width: 0;
    padding: 17px;
    border: 1px solid #e1e5e8;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.82);
    box-shadow: 0 10px 30px rgba(45, 57, 68, 0.045);
  }
  .nick-card > header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-bottom: 14px;
  }
  .nick-card header h3 {
    margin: 0;
    color: #272b30;
    font-size: 16px;
    font-weight: 650;
  }
  .nick-bot-list {
    display: grid;
    gap: 11px;
  }
  .nick-bot-list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .nick-section-title {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 9px;
    color: #555c63;
    font-size: 13px;
    font-weight: 650;
  }
  .nick-section-icon {
    width: 29px;
    height: 29px;
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 9px;
    color: #278dde;
    background: #edf7ff;
  }
  .nick-section-title-copy {
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .nick-section-title-copy strong {
    color: #34393e;
    font-size: 13px;
  }
  .nick-section-title small {
    color: #92979c;
    font-size: 11px;
    font-weight: 500;
  }
  .nick-bot-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 7px;
    margin-left: auto;
  }
  .nick-bot-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    gap: 9px;
  }
  .nick-bot-card {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px;
    border: 1px solid #e2e5e8;
    border-radius: 12px;
    color: #4c535a;
    background: rgba(255, 255, 255, 0.72);
    text-align: left;
  }
  .nick-bot-card:hover:not(:disabled),
  .nick-bot-card.selected {
    border-color: #83c2f7;
    background: #f2f9ff;
  }
  .nick-bot-card:disabled {
    opacity: 0.62;
    cursor: default;
  }
  .nick-bot-avatar {
    width: 31px;
    height: 31px;
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 9px;
    color: #9ea7ae;
    background: #eef1f3;
  }
  .nick-bot-card.online .nick-bot-avatar {
    color: #389df6;
    background: #e9f5ff;
  }
  .nick-bot-copy {
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .nick-bot-copy strong,
  .nick-bot-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nick-bot-copy strong {
    color: #34393e;
    font-size: 13px;
  }
  .nick-bot-copy small {
    color: #92979c;
    font-size: 11px;
  }
  .nick-selected-mark {
    margin-left: auto;
    color: #389df6;
  }
  .nick-field-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 11px;
  }
  .nick-field-grid label,
  .nick-list-grid label {
    display: grid;
    gap: 6px;
    color: #5b6268;
    font-size: 12px;
  }
  .nick-field-grid input,
  .nick-list-grid textarea {
    min-height: 35px;
    border: 1px solid #dfe3e6;
    border-radius: 9px;
    color: #34393e;
    background: #fff;
    font-size: 12px;
  }
  .nick-field-grid input {
    padding: 0 9px;
  }
  .nick-list-grid textarea {
    width: 100%;
    padding: 8px 9px;
    resize: vertical;
  }
  .nick-toggle-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px 20px;
    margin: 15px 0;
    padding: 12px 0;
    border-top: 1px solid #edf0f2;
    border-bottom: 1px solid #edf0f2;
  }
  .nick-toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: #555c63;
    font-size: 12px;
  }
  .nick-switch {
    width: 31px;
    height: 19px;
    min-height: 19px;
    padding: 2px;
    border: 0;
    border-radius: 20px;
    background: #d7dde2;
  }
  .nick-switch span {
    width: 15px;
    height: 15px;
    display: block;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.18);
    transform: translateX(0);
    transition: transform 0.16s ease;
  }
  .nick-switch.on {
    background: #389df6;
  }
  .nick-switch.on span {
    transform: translateX(12px);
  }
  .nick-list-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 11px;
  }
  .nick-list-grid label > span {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 5px;
  }
  .nick-priority {
    min-height: 22px;
    padding: 2px 7px;
    border-radius: 7px;
    color: #8c969f;
    font-size: 10px;
  }
  .nick-priority.on {
    color: #fff;
    border-color: #389df6;
    background: #389df6;
  }
  .nick-segmented {
    display: flex;
    gap: 3px;
    margin-top: 5px;
  }
  .nick-segmented button {
    flex: 1;
    min-height: 26px;
    padding: 3px 7px;
    border: 1px solid #dfe3e6;
    border-radius: 7px;
    color: #777f86;
    background: #fff;
    font-size: 11px;
  }
  .nick-segmented button.active {
    color: #fff;
    border-color: #389df6;
    background: #389df6;
  }
  .nick-candidate-card {
    min-height: 245px;
    display: flex;
    flex-direction: column;
  }
  .nick-candidate-card header {
    margin-bottom: 8px;
  }
  .nick-live-mark {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-left: auto;
    color: #7d8790;
    font-size: 11px;
  }
  .nick-live-mark span {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #34b776;
  }
  .nick-candidate-name {
    margin: 20px 0 5px;
    color: #278dde;
    font-size: clamp(23px, 3vw, 34px);
    font-weight: 750;
    letter-spacing: 0.02em;
    text-align: center;
  }
  .nick-candidate-list {
    display: grid;
    gap: 12px;
    max-height: 355px;
    overflow-y: auto;
  }
  .nick-candidate-item {
    padding-bottom: 12px;
    border-bottom: 1px solid #edf0f2;
  }
  .nick-candidate-item:last-child {
    padding-bottom: 0;
    border-bottom: 0;
  }
  .nick-candidate-item > small {
    display: block;
    color: #7f8991;
    font-size: 10px;
    text-align: center;
  }
  .nick-candidate-item .nick-candidate-name {
    margin-top: 7px;
    font-size: clamp(20px, 2.5vw, 30px);
  }
  .nick-candidate-reason {
    min-height: 18px;
    margin: 0;
    overflow: hidden;
    color: #8b949c;
    font-size: 11px;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nick-decision-countdown {
    margin: 9px 0;
    color: #d49125;
    font-size: 11px;
    text-align: center;
  }
  .nick-candidate-actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: auto;
  }
  .nick-candidate-actions button {
    padding: 6px 8px;
  }
  .nick-candidate-actions .nick-take {
    border-color: #389df6;
  }
  .nick-candidate-actions .nick-skip {
    color: #67717a;
    background: #f8fafb;
  }
  .nick-empty-candidate {
    min-height: 130px;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 8px;
    margin: auto 0;
    color: #9ca4aa;
    font-size: 12px;
    text-align: center;
  }
  .nick-stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 5px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #edf0f2;
  }
  .nick-stats span {
    display: grid;
    gap: 3px;
    color: #858e96;
    font-size: 10px;
    text-align: center;
  }
  .nick-stats strong {
    color: #278dde;
    font-size: 20px;
    line-height: 1;
  }
  .success-text {
    color: #2eae71 !important;
  }
  .danger-text {
    color: #dd655b !important;
  }
  .nick-sound-card header {
    align-items: center;
  }
  .nick-sound-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #edf0f2;
    color: #5b6268;
    font-size: 12px;
  }
  .nick-icon-toggle {
    min-height: 26px;
    width: 30px;
    padding: 0;
    color: #389df6;
    border: 0;
    background: transparent;
  }
  .nick-volume {
    display: grid;
    gap: 5px;
    margin-top: 7px;
    color: #89929a;
    font-size: 10px;
  }
  .nick-volume input {
    height: 4px;
    min-height: 4px;
    padding: 0;
    accent-color: #389df6;
  }
  .nick-secure-note {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 12px 0 0;
    color: #2eae71;
    font-size: 10px;
  }
  .nick-feedback-grid {
    display: grid;
    gap: 8px;
  }
  .nick-feedback {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 13px;
    border: 1px solid #bee8d0;
    border-radius: 12px;
    color: #288d5f;
    background: #f3fcf7;
  }
  .nick-feedback.failure,
  .nick-feedback.warning {
    border-color: #f0d19b;
    color: #b37415;
    background: #fffaf0;
  }
  .nick-feedback.failure {
    border-color: #f0c2be;
    color: #c95249;
    background: #fff7f6;
  }
  .nick-feedback div {
    min-width: 0;
    display: grid;
    gap: 3px;
  }
  .nick-feedback strong {
    font-size: 12px;
  }
  .nick-feedback span {
    color: currentColor;
    opacity: 0.82;
    font-size: 11px;
  }
  .nick-activity-card header {
    align-items: center;
  }
  .nick-activity-list {
    display: grid;
    max-height: 230px;
    overflow-y: auto;
  }
  .nick-activity-row {
    min-height: 35px;
    display: grid;
    grid-template-columns: 23px minmax(0, 1fr) auto;
    align-items: center;
    gap: 7px;
    border-bottom: 1px solid #f0f2f3;
    color: #5d666e;
    font-size: 12px;
  }
  .nick-activity-row small {
    color: #9aa2a8;
    font-size: 10px;
  }
  .nick-activity-row.success {
    color: #2eae71;
  }
  .nick-activity-row.danger {
    color: #d45a52;
  }
  .nick-activity-row.muted {
    color: #9099a0;
  }
  .nick-activity-icon {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: #389df6;
    background: #edf7ff;
  }
  .nick-empty,
  .nick-empty-activity {
    color: #92979c;
    font-size: 12px;
  }
  .nick-empty-activity {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 18px 0;
  }
  @media (max-width: 980px) {
    .nick-main-grid {
      grid-template-columns: 1fr;
    }
    .nick-operation-grid .nick-side-column,
    .nick-side-column {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 720px) {
    .nick-bot-list-header {
      align-items: stretch;
      flex-direction: column;
    }
    .nick-bot-actions {
      margin-left: 0;
      justify-content: flex-start;
    }
    .nick-field-grid,
    .nick-list-grid,
    .nick-operation-grid .nick-side-column,
    .nick-side-column {
      grid-template-columns: 1fr;
    }
    .nick-toggle-grid {
      grid-template-columns: 1fr;
    }
  }

  :global(html[data-theme="dark"]) .nick-card header h3,
  :global(html[data-theme="dark"]) .nick-bot-copy strong,
  :global(html[data-theme="dark"]) .nick-section-title-copy strong {
    color: #edf1ef;
  }
  :global(html[data-theme="dark"]) .nick-card header p,
  :global(html[data-theme="dark"]) .nick-section-title,
  :global(html[data-theme="dark"]) .nick-field-grid label,
  :global(html[data-theme="dark"]) .nick-list-grid label,
  :global(html[data-theme="dark"]) .nick-toggle-row,
  :global(html[data-theme="dark"]) .nick-sound-row {
    color: #aeb8b2;
  }
  :global(html[data-theme="dark"]) .nick-card {
    border-color: rgba(255, 255, 255, 0.09);
    color: #e4ebe7;
    background: rgba(31, 36, 32, 0.72);
    box-shadow:
      0 12px 28px rgba(0, 0, 0, 0.18),
      inset 0 1px rgba(255, 255, 255, 0.035);
  }
  :global(html[data-theme="dark"]) .nick-section-icon {
    color: #7bc8f5;
    background: rgba(42, 132, 191, 0.2);
  }
  :global(html[data-theme="dark"]) .nick-bot-card {
    border-color: rgba(255, 255, 255, 0.1);
    color: #c0cbc3;
    background: rgba(29, 35, 31, 0.78);
  }
  :global(html[data-theme="dark"]) .nick-bot-card:hover:not(:disabled),
  :global(html[data-theme="dark"]) .nick-bot-card.selected {
    border-color: #389df6;
    background: rgba(42, 133, 191, 0.24);
  }
  :global(html[data-theme="dark"]) .nick-bot-copy small,
  :global(html[data-theme="dark"]) .nick-candidate-reason,
  :global(html[data-theme="dark"]) .nick-section-title small,
  :global(html[data-theme="dark"]) .nick-empty,
  :global(html[data-theme="dark"]) .nick-empty-candidate,
  :global(html[data-theme="dark"]) .nick-empty-activity {
    color: #7f8983;
  }
  :global(html[data-theme="dark"]) .nick-bot-avatar {
    color: #83a6bd;
    background: rgba(255, 255, 255, 0.08);
  }
  :global(html[data-theme="dark"]) .nick-bot-card.online .nick-bot-avatar {
    color: #7bc8f5;
    background: rgba(42, 132, 191, 0.2);
  }
  :global(html[data-theme="dark"]) .nick-field-grid input,
  :global(html[data-theme="dark"]) .nick-list-grid textarea {
    border-color: rgba(255, 255, 255, 0.11);
    color: #e5ebe7;
    background: rgba(13, 16, 14, 0.72);
  }
  :global(html[data-theme="dark"]) .nick-field-grid input::placeholder,
  :global(html[data-theme="dark"]) .nick-list-grid textarea::placeholder {
    color: #737e77;
  }
  :global(html[data-theme="dark"]) .nick-toggle-grid,
  :global(html[data-theme="dark"]) .nick-stats,
  :global(html[data-theme="dark"]) .nick-sound-row,
  :global(html[data-theme="dark"]) .nick-candidate-item {
    border-color: rgba(255, 255, 255, 0.09);
  }
  :global(html[data-theme="dark"]) .nick-priority,
  :global(html[data-theme="dark"]) .nick-segmented button,
  :global(html[data-theme="dark"]) .nick-candidate-actions .nick-skip,
  :global(html[data-theme="dark"]) .nick-save-button {
    border-color: rgba(255, 255, 255, 0.11);
    color: #c8d0cb;
    background: rgba(36, 41, 37, 0.78);
  }
  :global(html[data-theme="dark"]) .nick-priority.on,
  :global(html[data-theme="dark"]) .nick-segmented button.active,
  :global(html[data-theme="dark"]) .nick-take,
  :global(html[data-theme="dark"]) .nick-save-button {
    color: #fff;
    border-color: #389df6;
    background: #389df6;
  }
  :global(html[data-theme="dark"]) .nick-action-button:not(.danger) {
    color: #fff;
    border-color: #389df6;
    background: #389df6;
  }
  :global(html[data-theme="dark"]) .nick-action-button.danger {
    border-color: #b9423d;
    background: #c94742;
  }
  :global(html[data-theme="dark"]) .nick-activity-row {
    border-bottom-color: rgba(255, 255, 255, 0.085);
    color: #d6ddd8;
  }
  :global(html[data-theme="dark"]) .nick-activity-row.muted {
    color: #929d97;
  }
</style>
