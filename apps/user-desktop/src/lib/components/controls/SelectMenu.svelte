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
  // 提供可收藏的選單選項；關閉時移除選項的鍵盤焦點順序。
  import { onMount } from "svelte";
  import { ChevronDown, Star } from "@lucide/svelte";

  export let value: string;
  export let options: Array<{ value: string; label: string }>;
  export let ariaLabel: string;
  export let onSelect: (value: string) => void;
  export let disabled = false;
  export let maxVisibleOptions: number | null = null;
  export let favoriteValues: ReadonlySet<string> | null = null;
  export let onToggleFavorite: ((value: string) => void) | null = null;
  export let addFavoriteLabel = "Add to favorites";
  export let removeFavoriteLabel = "Remove from favorites";

  let root: HTMLDivElement;
  let open = false;
  $: selectedLabel =
    options.find((option) => option.value === value)?.label ?? options[0]?.label ?? "";

  function choose(nextValue: string) {
    if (disabled) return;
    onSelect(nextValue);
    open = false;
  }

  function handleTriggerKeydown(event: KeyboardEvent) {
    if (["ArrowDown", "Enter", " "].includes(event.key)) {
      event.preventDefault();
      open = true;
    } else if (event.key === "Escape") {
      open = false;
    }
  }

  onMount(() => {
    const closeOutside = (event: PointerEvent) => {
      if (open && !root.contains(event.target as Node)) open = false;
    };
    document.addEventListener("pointerdown", closeOutside);
    return () => document.removeEventListener("pointerdown", closeOutside);
  });
</script>

<div class="select-menu" class:open bind:this={root}>
  <button
    type="button"
    class="select-trigger"
    aria-label={ariaLabel}
    aria-haspopup="listbox"
    aria-expanded={open}
    {disabled}
    on:click={() => (open = !open)}
    on:keydown={handleTriggerKeydown}
  >
    <span>{selectedLabel}</span>
    <span class="select-chevron"><ChevronDown size={15} /></span>
  </button>
  <div class="select-options" aria-hidden={!open}>
    <div class="select-options-clip">
      <div
        class="select-options-surface"
        role="listbox"
        aria-label={ariaLabel}
        style={maxVisibleOptions
          ? `max-height: min(${maxVisibleOptions * 37 + 7}px, 55vh)`
          : undefined}
      >
        {#each options as option}
          <div class="select-option" class:selected={option.value === value}>
            <button
              type="button"
              class="option-choice"
              role="option"
              aria-selected={option.value === value}
              {disabled}
              tabindex={open ? 0 : -1}
              on:click={() => choose(option.value)}
            >
              <span class="option-marker"></span>
              <span>{option.label}</span>
            </button>
            {#if favoriteValues && onToggleFavorite}
              <button
                type="button"
                class="option-favorite"
                class:favorite={favoriteValues.has(option.value)}
                aria-label={favoriteValues.has(option.value)
                  ? removeFavoriteLabel
                  : addFavoriteLabel}
                title={favoriteValues.has(option.value) ? removeFavoriteLabel : addFavoriteLabel}
                {disabled}
                tabindex={open ? 0 : -1}
                on:click={() => onToggleFavorite?.(option.value)}
              >
                <Star size={15} fill={favoriteValues.has(option.value) ? "currentColor" : "none"} />
              </button>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>

<style>
  .select-menu {
    position: relative;
    width: 100%;
    min-width: min(170px, 100%);
    max-width: 100%;
  }
  .select-trigger {
    width: 100%;
    height: 40px;
    justify-content: space-between;
    padding: 0 11px 0 13px;
    border: 1px solid rgba(174, 188, 202, 0.58);
    border-radius: 11px;
    color: #3f4b56;
    background: rgba(255, 255, 255, 0.76);
    box-shadow: inset 0 1px rgba(255, 255, 255, 0.66);
    transition:
      border-color 0.2s ease,
      background-color 0.2s ease,
      box-shadow 0.2s ease;
  }
  .select-trigger:hover {
    border-color: rgba(131, 161, 188, 0.68);
    background: rgba(255, 255, 255, 0.9);
  }
  .select-trigger:disabled {
    cursor: not-allowed;
    opacity: 0.58;
  }
  .select-trigger:focus-visible {
    outline: 0;
    border-color: #389df6;
    box-shadow: 0 0 0 3px rgba(56, 157, 246, 0.13);
  }
  .select-trigger span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .select-chevron {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    color: #788694;
    transition: transform var(--motion-duration-menu) var(--motion-ease-out);
  }
  .open .select-chevron {
    transform: rotate(180deg);
  }
  .select-options {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transform: translate3d(0, -5px, 0);
    clip-path: inset(0 0 100% 0);
    pointer-events: none;
    transition:
      opacity var(--motion-duration-menu) var(--motion-ease-out),
      transform var(--motion-duration-menu) var(--motion-ease-out),
      clip-path var(--motion-duration-menu) var(--motion-ease-out);
  }
  .select-menu.open .select-options {
    grid-template-rows: 1fr;
    opacity: 1;
    transform: translate3d(0, 0, 0);
    clip-path: inset(0);
    pointer-events: auto;
  }
  .select-options-clip {
    min-height: 0;
    overflow: hidden;
  }
  .select-options-surface {
    display: grid;
    gap: 3px;
    margin-top: 6px;
    padding: 5px;
    border-radius: 11px;
    background: #f9fbfd;
    box-shadow:
      0 12px 26px rgba(47, 68, 88, 0.1),
      inset 0 1px rgba(255, 255, 255, 0.82);
    max-height: min(360px, 55vh);
    overflow-y: auto;
  }
  .select-option {
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    border-radius: 8px;
    transition:
      color 0.18s ease,
      background-color 0.18s ease;
  }
  .select-option:hover {
    color: #273745;
    background: rgba(230, 238, 245, 0.62);
  }
  .select-option.selected {
    color: #167fd5;
    background: rgba(224, 241, 255, 0.72);
  }
  .option-choice {
    width: 100%;
    height: 34px;
    justify-content: flex-start;
    gap: 9px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    color: #626d78;
    background: transparent;
    box-shadow: none;
    transition: color 0.18s ease;
  }
  .option-choice:hover {
    color: inherit;
    border-color: transparent;
    background: transparent;
  }
  .option-choice span:last-child {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .option-favorite {
    width: 30px;
    height: 30px;
    margin-right: 2px;
    padding: 0;
    border: 0;
    border-radius: 7px;
    color: #a3adb6;
    background: transparent;
    box-shadow: none;
  }
  .option-favorite:hover {
    color: #e9a91b;
    border-color: transparent;
    background: rgba(255, 194, 57, 0.11);
  }
  .option-favorite.favorite {
    color: #e9a91b;
  }
  .option-marker {
    width: 7px;
    height: 7px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: transparent;
  }
  .select-option.selected .option-marker {
    background: #389df6;
    box-shadow: 0 0 0 3px rgba(56, 157, 246, 0.12);
  }

  :global(html[data-theme="dark"]) .select-trigger {
    color: #dce4df;
    border-color: rgba(255, 255, 255, 0.11);
    background: rgba(13, 16, 14, 0.72);
    box-shadow: inset 0 1px rgba(255, 255, 255, 0.035);
  }
  :global(html[data-theme="dark"]) .select-trigger:hover {
    border-color: rgba(82, 174, 231, 0.38);
    background: rgba(32, 38, 34, 0.9);
  }
  :global(html[data-theme="dark"]) .select-options-surface {
    background: rgba(24, 29, 25, 0.98);
    box-shadow:
      0 14px 28px rgba(0, 0, 0, 0.32),
      inset 0 1px rgba(255, 255, 255, 0.04);
  }
  :global(html[data-theme="dark"]) .select-option {
    color: #aeb8b2;
  }
  :global(html[data-theme="dark"]) .select-option:hover {
    color: #edf1ef;
    background: rgba(255, 255, 255, 0.055);
  }
  :global(html[data-theme="dark"]) .select-option.selected {
    color: #78c2f0;
    background: rgba(38, 128, 187, 0.18);
  }
  :global(html[data-theme="dark"]) .option-choice,
  :global(html[data-theme="dark"]) .option-choice:hover,
  :global(html[data-theme="dark"]) .option-favorite {
    color: inherit;
    border-color: transparent;
    background: transparent;
  }
  :global(html[data-theme="dark"]) .option-favorite:hover,
  :global(html[data-theme="dark"]) .option-favorite.favorite {
    color: #f0b83e;
    background: rgba(240, 184, 62, 0.1);
  }

  @media (prefers-reduced-motion: reduce) {
    .select-chevron {
      transition: none !important;
      transform: none !important;
    }

    .select-options {
      transition: opacity 120ms linear !important;
      transform: none !important;
      clip-path: inset(0) !important;
    }
  }
</style>
