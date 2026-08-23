<script lang="ts">
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
        // Keep a NameMC-style three-quarter viewing angle.
        viewer.playerWrapper.rotation.y = 0.5;
        viewer.camera.position.y = 18;
        viewer.camera.lookAt(0, 0, 0);
        reducedMotion.addEventListener("change", applyMotionPreference);
        removeMotionListener = () => reducedMotion.removeEventListener("change", applyMotionPreference);
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

<span class="minecraft-skin" style:width={`${canvasWidth}px`} style:height={`${size}px`} title={username}>
  <canvas class:hidden={!skinSource || loadError} bind:this={canvas} aria-label={`${username} Minecraft skin`}></canvas>
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
    filter: drop-shadow(0 0 3px rgba(32, 47, 60, .3));
    image-rendering: pixelated;
    object-fit: contain;
  }
  canvas.hidden { display: none; }
  .minecraft-skin-placeholder {
    width: 52%;
    height: 88%;
    border-radius: 3px;
    background: linear-gradient(#9ca8b2 0 18%, #6f7d88 18% 58%, #53616c 58% 100%);
    box-shadow: inset 0 0 0 1px rgba(50, 64, 75, .18);
  }
</style>
