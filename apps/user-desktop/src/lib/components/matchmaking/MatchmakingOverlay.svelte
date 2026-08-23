<script lang="ts">
  import { onMount } from "svelte";
  import { Activity, Check, EyeOff, Radio, X } from "@lucide/svelte";
  import type { Account, BotMatchPhase, MatchmakingPhase, MatchmakingSnapshot } from "../../models/types";

  const OVERLAY_REFERENCE_SCALE = 1.25;
  const OVERLAY_BASE_HEIGHT = 179;
  const OVERLAY_BOT_HEIGHT = 57;
  const OVERLAY_MAX_VISIBLE_BOTS = 5;

  export let snapshot: MatchmakingSnapshot;
  export let accounts: Account[];
  export let phaseLabel: (phase: MatchmakingPhase) => string;
  export let modeLabel: (mode: MatchmakingSnapshot["mode"]) => string;
  export let attemptsLabel: string;
  export let matchedLabel: string;
  export let minimumLabel: string;
  export let playerServerLabel: string;
  export let queueingLabel: string;
  export let hideLabel: string;
  export let animationKey = 0;
  export let onHide: () => void;

  let devicePixelRatio = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;

  $: progressMax = Math.max(1, snapshot.bots.length);
  $: progressRatio = Math.min(1, Math.max(0, snapshot.matched_bots / progressMax));
  $: visibleBots = Math.min(Math.max(snapshot.bots.length, 1), OVERLAY_MAX_VISIBLE_BOTS);
  $: designHeight = OVERLAY_BASE_HEIGHT + visibleBots * OVERLAY_BOT_HEIGHT;
  $: designScale = OVERLAY_REFERENCE_SCALE / devicePixelRatio;

  onMount(() => {
    const updateDevicePixelRatio = () => {
      devicePixelRatio = window.devicePixelRatio || 1;
    };

    updateDevicePixelRatio();
    window.addEventListener("resize", updateDevicePixelRatio);
    return () => window.removeEventListener("resize", updateDevicePixelRatio);
  });

  function botLabel(botId: string): string {
    const index = accounts.findIndex((account) => account.id === botId);
    return index < 0 ? botId : `Bot-${String(index + 1).padStart(2, "0")}`;
  }

  function resultClass(phase: BotMatchPhase): string {
    if (phase === "matched" || phase === "afk") return "matched";
    if (phase === "unavailable") return "failed";
    return "working";
  }

</script>

<main
  class="match-overlay"
  style={`--match-overlay-design-height: ${designHeight}px; --match-overlay-scale: ${designScale};`}
>
  <div class="match-overlay-frame">
    {#key animationKey}
      <div class="match-overlay-content">
        <header class="match-overlay-titlebar" data-tauri-drag-region>
          <div data-tauri-drag-region>
            <span class="match-overlay-mark"><Radio size={15} /></span>
            <div class="match-overlay-heading" data-tauri-drag-region>
              <strong data-tauri-drag-region>Matchmaking</strong>
              <small data-tauri-drag-region>{modeLabel(snapshot.mode)}</small>
            </div>
          </div>
          <button title={hideLabel} aria-label={hideLabel} on:click={onHide}><EyeOff size={16} /></button>
        </header>

        <section class="match-overlay-summary">
          <div class="match-overlay-count">
            <strong>{snapshot.matched_bots}</strong><span>/{snapshot.bots.length}</span>
          </div>
          <div class="match-overlay-progress-copy">
            <div><span>{matchedLabel}</span><span>{minimumLabel} {snapshot.required_matches}</span></div>
            <div
              class="match-overlay-progress"
              role="progressbar"
              aria-valuemin="0"
              aria-valuemax={progressMax}
              aria-valuenow={snapshot.matched_bots}
              aria-label={matchedLabel}
            >
              <span style={`transform: scaleX(${progressRatio});`}></span>
            </div>
          </div>
        </section>

        <div class="match-overlay-player">
          <Radio size={15} />
          <span>{playerServerLabel}</span>
          <strong>{snapshot.player_server || "—"}</strong>
        </div>

        <section class="match-overlay-bots" aria-live="polite">
          {#each snapshot.bots as bot (bot.bot_id)}
            <article class={resultClass(bot.phase)}>
              <span class="match-overlay-result">
                {#if bot.phase === "matched" || bot.phase === "afk"}<Check size={15} />
                {:else if bot.phase === "unavailable"}<X size={15} />
                {:else}<Activity size={14} />{/if}
              </span>
              <div>
                <strong>{botLabel(bot.bot_id)}</strong>
                <small>
                  {#if bot.phase === "queued" || bot.phase === "returning"}
                    {queueingLabel}<span class="queueing-dots" aria-hidden="true"></span>
                  {:else}
                    {bot.server || bot.phase}
                  {/if}
                  · {bot.attempts} {attemptsLabel}
                </small>
              </div>
            </article>
          {:else}
            <p class="match-overlay-empty">{phaseLabel(snapshot.phase)}</p>
          {/each}
        </section>
      </div>
    {/key}
  </div>

</main>
