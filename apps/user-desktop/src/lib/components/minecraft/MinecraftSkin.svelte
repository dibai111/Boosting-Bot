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
  // 延遲載入 3D 皮膚檢視器，失敗時改用靜態圖片並在卸載時釋放檢視器。
  import { onMount } from "svelte";
  import type { SkinViewer as SkinViewerInstance } from "skinview3d";

  export let username = "";
  export let profileId = "";
  export let size = 82;

  const skinApiBase = "https://visage.surgeplay.com";

  let canvas: HTMLCanvasElement;
  let viewer: SkinViewerInstance | null = null;
  let loadError = false;
  let requestId = 0;
  let activeSource = "";

  $: skinUuid = profileId.trim();
  $: skinSource = skinUuid ? `${skinApiBase}/skin/${encodeURIComponent(skinUuid)}.png` : "";
  $: canvasWidth = Math.round(size * 0.72);

  /**
   * 載入皮膚並忽略已被較新請求取代的錯誤。
   * @param source 皮膚 PNG 網址。
   * @return 載入流程的 Promise；失敗時啟用靜態替代圖片。
   */
  async function loadSkin(source: string) {
    if (!viewer || !source) return;
    const currentViewer = viewer;
    const currentRequest = ++requestId;
    loadError = false;

    try {
      await currentViewer.loadSkin(source, { model: "auto-detect" });
    } catch {
      if (currentRequest === requestId && currentViewer === viewer) loadError = true;
    }
  }

  onMount(() => {
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let removeMotionListener: (() => void) | null = null;
    let disposed = false;

    void (async () => {
      try {
        const { SkinViewer, WalkingAnimation } = await import("skinview3d");
        if (disposed) return;
        const createWalkingAnimation = () => {
          const animation = new WalkingAnimation();
          animation.speed = 0.45;
          animation.headBobbing = false;
          return animation;
        };
        const applyMotionPreference = () => {
          if (!viewer) return;
          viewer.animation = reducedMotion.matches ? null : createWalkingAnimation();
        };

        viewer = new SkinViewer({
          canvas,
          width: canvasWidth,
          height: size,
          pixelRatio: Math.min(window.devicePixelRatio || 1, 2),
          enableControls: false,
          zoom: 0.98,
          animation: reducedMotion.matches ? undefined : createWalkingAnimation(),
        });
        // 以略側身的視角同時呈現皮膚正面與側面。
        viewer.playerWrapper.rotation.y = 0.5;
        viewer.camera.position.y = 18;
        viewer.camera.lookAt(0, 0, 0);
        reducedMotion.addEventListener("change", applyMotionPreference);
        removeMotionListener = () =>
          reducedMotion.removeEventListener("change", applyMotionPreference);
        activeSource = skinSource;
        void loadSkin(skinSource);
      } catch {
        if (!disposed) loadError = true;
      }
    })();

    return () => {
      disposed = true;
      removeMotionListener?.();
      viewer?.dispose();
      viewer = null;
    };
  });

  $: if (viewer && skinSource !== activeSource) {
    activeSource = skinSource;
    void loadSkin(skinSource);
  }

  $: if (viewer) viewer.setSize(canvasWidth, size);
</script>

<span
  class="minecraft-skin"
  style:width={`${canvasWidth}px`}
  style:height={`${size}px`}
  title={username}
>
  <canvas
    class:hidden={!skinSource || loadError}
    bind:this={canvas}
    aria-label={`${username} Minecraft skin`}
  ></canvas>
  {#if loadError && skinUuid}
    <img
      class="minecraft-skin-fallback"
      src={`${skinApiBase}/full/${Math.max(64, size * 2)}/${encodeURIComponent(skinUuid)}`}
      alt={`${username} Minecraft skin`}
      draggable="false"
    />
  {:else if !skinSource || loadError}
    <span class="minecraft-skin-placeholder" aria-hidden="true"></span>
  {/if}
</span>

<style>
  .minecraft-skin {
    position: relative;
    flex: 0 0 auto;
    display: inline-grid;
    place-items: center;
    overflow: visible;
  }
  canvas,
  .minecraft-skin-fallback {
    width: 100%;
    height: 100%;
    display: block;
    filter: drop-shadow(0 0 3px rgba(32, 47, 60, 0.3));
    image-rendering: pixelated;
    object-fit: contain;
  }
  canvas.hidden {
    display: none;
  }
  .minecraft-skin-placeholder {
    width: 52%;
    height: 88%;
    border-radius: 3px;
    background: linear-gradient(#9ca8b2 0 18%, #6f7d88 18% 58%, #53616c 58% 100%);
    box-shadow: inset 0 0 0 1px rgba(50, 64, 75, 0.18);
  }
</style>
