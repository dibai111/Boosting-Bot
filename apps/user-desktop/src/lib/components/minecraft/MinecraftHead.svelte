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
  // 載入玩家頭像，請求失敗或沒有名稱時顯示替代圖示。
  import { UserRound } from "@lucide/svelte";

  export let username: string;
  export let size = 28;
  let failed = false;

  $: if (username) failed = false;
  $: source = `https://mc-heads.net/avatar/${encodeURIComponent(username)}/${Math.max(16, size * 2)}`;
</script>

<span class="minecraft-head" style:width={`${size}px`} style:height={`${size}px`} title={username}>
  {#if username && !failed}
    <img src={source} alt="" draggable="false" on:error={() => (failed = true)} />
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
    background: rgba(132, 148, 162, 0.14);
    box-shadow: inset 0 0 0 1px rgba(102, 118, 132, 0.18);
  }
  img {
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    object-fit: cover;
  }
</style>
