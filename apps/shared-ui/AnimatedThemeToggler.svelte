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
  // 切換明暗主題並從按鈕中心展開動畫；不支援動畫時直接套用主題。
  import { onDestroy } from "svelte";
  type Theme = "light" | "dark";
  type ViewTransitionController = { ready: Promise<void> };
  type DocumentWithViewTransition = Document & {
    startViewTransition?: (callback: () => void) => ViewTransitionController;
  };

  export let theme: Theme = "light";
  export let onToggle: () => void = () => undefined;
  export let lightLabel = "Use light theme";
  export let darkLabel = "Use dark theme";

  let button: HTMLButtonElement;
  let activeRevealAnimation: Animation | null = null;
  let toggleRequest = 0;

  onDestroy(() => {
    // 使尚未完成的 ready 回呼失效，避免卸載後再啟動根元素動畫。
    toggleRequest += 1;
    activeRevealAnimation?.cancel();
    activeRevealAnimation = null;
  });

  /**
   * 切換主題，支援時從按鈕中心展開 View Transition。
   * @return 動畫準備的 Promise；不支援或偏好減少動態時直接切換。
   */
  async function toggleTheme(): Promise<void> {
    const requestId = ++toggleRequest;
    activeRevealAnimation?.cancel();
    activeRevealAnimation = null;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const startViewTransition = (document as DocumentWithViewTransition).startViewTransition;
    if (reducedMotion || !startViewTransition) {
      onToggle();
      return;
    }

    let transition: ViewTransitionController;
    try {
      // Web API 需要以 document 作為接收者，抽出函式後仍須保留 this。
      transition = startViewTransition.call(document, () => onToggle());
    } catch {
      onToggle();
      return;
    }

    try {
      await transition.ready;
    } catch {
      return;
    }

    if (requestId !== toggleRequest || !button) return;
    const bounds = button.getBoundingClientRect();
    const x = bounds.left + bounds.width / 2;
    const y = bounds.top + bounds.height / 2;
    const radius = Math.hypot(
      Math.max(x, window.innerWidth - x),
      Math.max(y, window.innerHeight - y),
    );
    const animationOptions: KeyframeAnimationOptions & { pseudoElement: string } = {
      duration: 280,
      easing: "cubic-bezier(.23, 1, .32, 1)",
      pseudoElement: "::view-transition-new(root)",
    };

    try {
      const animation = document.documentElement.animate(
        {
          clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`],
        },
        animationOptions,
      );
      activeRevealAnimation = animation;
      const clearAnimation = () => {
        if (activeRevealAnimation === animation) activeRevealAnimation = null;
      };
      void animation.finished.then(clearAnimation, clearAnimation);
    } catch {
      // 舊版 WebView 可能支援主題切換，但不支援偽元素動畫；保留已套用的主題。
    }
  }
</script>

<button
  bind:this={button}
  type="button"
  class="animated-theme-toggler"
  class:dark={theme === "dark"}
  title={theme === "dark" ? lightLabel : darkLabel}
  aria-label={theme === "dark" ? lightLabel : darkLabel}
  aria-pressed={theme === "dark"}
  on:click={() => void toggleTheme()}
>
  <span class="animated-theme-icon" class:visible={theme !== "dark"} aria-hidden="true"
    ><slot name="light-icon" /></span
  >
  <span class="animated-theme-icon" class:visible={theme === "dark"} aria-hidden="true"
    ><slot name="dark-icon" /></span
  >
</button>

<style>
  .animated-theme-toggler {
    position: relative;
    width: 36px;
    min-width: 36px;
    height: 36px;
    min-height: 36px;
    display: grid;
    place-items: center;
    padding: 0;
    overflow: hidden;
    isolation: isolate;
    border: 1px solid rgba(118, 157, 188, 0.34);
    border-radius: 11px;
    color: #2b82c8;
    background: rgba(255, 255, 255, 0.56);
    box-shadow:
      0 5px 14px rgba(49, 121, 178, 0.1),
      inset 0 1px rgba(255, 255, 255, 0.8);
    transition:
      color 0.28s ease,
      border-color 0.28s ease,
      background-color 0.28s ease,
      transform 0.22s ease,
      box-shadow 0.28s ease;
  }

  .animated-theme-toggler::before {
    position: absolute;
    z-index: -1;
    inset: 3px;
    border-radius: 8px;
    background: linear-gradient(145deg, rgba(204, 228, 255, 0.9), rgba(255, 255, 255, 0.22));
    content: "";
    opacity: 0.82;
    transform: scale(0.72);
    transition:
      opacity 220ms cubic-bezier(0.23, 1, 0.32, 1),
      transform 220ms cubic-bezier(0.23, 1, 0.32, 1),
      background 220ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .animated-theme-toggler:hover {
    border-color: rgba(56, 157, 246, 0.64);
    box-shadow:
      0 8px 18px rgba(49, 121, 178, 0.16),
      inset 0 1px rgba(255, 255, 255, 0.9);
  }

  @media (hover: hover) and (pointer: fine) {
    .animated-theme-toggler:hover {
      transform: translateY(-1px);
    }
  }

  .animated-theme-toggler:hover::before,
  .animated-theme-toggler.dark::before {
    opacity: 1;
    transform: scale(1);
  }

  .animated-theme-toggler.dark {
    border-color: rgba(142, 193, 232, 0.3);
    color: #d6edff;
    background: rgba(17, 26, 34, 0.76);
    box-shadow:
      0 6px 16px rgba(0, 0, 0, 0.2),
      inset 0 1px rgba(255, 255, 255, 0.1);
  }

  .animated-theme-toggler.dark::before {
    background: linear-gradient(145deg, rgba(49, 113, 159, 0.72), rgba(18, 29, 40, 0.9));
  }

  .animated-theme-toggler:active:not(:disabled) {
    transform: translateY(1px) scale(0.97);
  }

  .animated-theme-icon {
    position: absolute;
    z-index: 1;
    display: grid;
    place-items: center;
    opacity: 0;
    transform: rotate(-10deg) scale(0.94);
    transition:
      opacity 220ms cubic-bezier(0.23, 1, 0.32, 1),
      transform 220ms cubic-bezier(0.23, 1, 0.32, 1);
  }

  .animated-theme-icon :global(svg) {
    display: block;
  }

  .animated-theme-icon.visible {
    opacity: 1;
    transform: rotate(0) scale(1);
  }

  @media (prefers-reduced-motion: reduce) {
    .animated-theme-toggler,
    .animated-theme-toggler::before,
    .animated-theme-icon {
      transition: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .animated-theme-toggler:hover,
    .animated-theme-toggler:active:not(:disabled) {
      transform: none !important;
    }
  }

  :global(::view-transition-old(root)),
  :global(::view-transition-new(root)) {
    animation: none;
  }
</style>
