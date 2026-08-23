<script lang="ts">
  import { UserRound } from "@lucide/svelte";

  export let username: string;
  export let size = 28;
  let failed = false;

  $: if (username) failed = false;
  $: source = `https://mc-heads.net/avatar/${encodeURIComponent(username)}/${Math.max(16, size * 2)}`;
</script>

<span class="minecraft-head" style:width={`${size}px`} style:height={`${size}px`} title={username}>
  {#if username && !failed}
    <img src={source} alt="" draggable="false" on:error={() => failed = true} />
  {:else}
    <UserRound size={Math.round(size * 0.62)} />
  {/if}
</span>

<style>
  .minecraft-head {
    flex: 0 0 auto;
    display: inline-grid;
    place-items: center;
    overflow: hidden;
    border-radius: 6px;
    color: #73808b;
    background: rgba(132, 148, 162, .14);
    box-shadow: inset 0 0 0 1px rgba(102, 118, 132, .18);
  }
  img { width: 100%; height: 100%; image-rendering: pixelated; object-fit: cover; }
</style>
