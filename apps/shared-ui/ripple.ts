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

/** 將點擊位置轉成按鈕漣漪；鍵盤觸發時改用按鈕中心。 */
export interface RippleOptions {
  rippleColor?: string;
  durationMs?: number;
}

type RippleElement = HTMLElement & { __bottingRippleCleanup?: () => void };

/**
 * 將滑鼠或鍵盤點擊轉成短暫漣漪。
 * @param node 套用效果的 DOM 元素。
 * @param options 漣漪顏色及持續時間。
 * @return Svelte action 的 update 與 destroy 介面。
 */
export function ripple(node: HTMLElement, options: RippleOptions = {}) {
  let currentOptions = options;
  const element = node as RippleElement;

  function applyOptions(next: RippleOptions): void {
    currentOptions = next;
    node.style.setProperty("--ripple-color", next.rippleColor ?? "#ADD8E6");
  }

  function handleClick(event: MouseEvent): void {
    if (node.matches(":disabled") || window.matchMedia("(prefers-reduced-motion: reduce)").matches)
      return;

    const bounds = node.getBoundingClientRect();
    const size = Math.max(bounds.width, bounds.height) * 2.2;
    const centerX = event.detail === 0 ? bounds.width / 2 : event.clientX - bounds.left;
    const centerY = event.detail === 0 ? bounds.height / 2 : event.clientY - bounds.top;
    const wave = document.createElement("span");
    wave.className = "ripple-wave";
    wave.style.width = `${size}px`;
    wave.style.height = `${size}px`;
    wave.style.left = `${centerX - size / 2}px`;
    wave.style.top = `${centerY - size / 2}px`;
    node.querySelectorAll(".ripple-wave").forEach((wave) => wave.remove());
    node.append(wave);

    const duration = Math.min(160, Math.max(100, currentOptions.durationMs ?? 140));
    window.setTimeout(() => wave.remove(), duration + 40);
  }

  applyOptions(currentOptions);
  node.classList.add("ripple-button");
  node.addEventListener("click", handleClick);
  element.__bottingRippleCleanup = () => {
    node.removeEventListener("click", handleClick);
    node.classList.remove("ripple-button");
    node.style.removeProperty("--ripple-color");
  };

  return {
    update: applyOptions,
    destroy: () => element.__bottingRippleCleanup?.(),
  };
}
