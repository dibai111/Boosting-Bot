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
  // 呈現 Bot 狀態並編輯伺服器地址；選取區與輸入欄分開接收操作。
  import MinecraftSkin from "../minecraft/MinecraftSkin.svelte";
  import HexagonPattern from "../../../../../shared-ui/HexagonPattern.svelte";

  export let username = "";
  export let profileId = "";
  export let serverAddress = "";
  export let serverAddressLabel = "Server address";
  export let status = "";
  export let statusTone = "gray";
  export let selected = false;
  export let onSelect: () => void = () => {};
  export let onServerChange: (value: string) => void = () => {};
</script>

<article class="bot-card" class:selected>
  <HexagonPattern
    className="identity-hexagon-pattern"
    hexagons={[
      [1, 1],
      [4, 4],
      [2, 2],
      [3, 4],
      [5, 4],
      [8, 2],
      [6, 3],
      [8, 5],
      [2, 5],
      [3, 2],
      [5, 2],
      [7, 1],
      [9, 3],
      [1, 4],
      [4, 1],
      [6, 5],
      [7, 5],
      [9, 5],
      [2, 6],
      [5, 6],
      [8, 4],
    ]}
  />
  <button
    type="button"
    class="bot-card-select-area"
    aria-label={username}
    aria-pressed={selected}
    on:click={onSelect}
  ></button>
  <header class="bot-card-header">
    <span
      class="account-card-status bot-card-status"
      class:status-dot-only={statusTone === "gray"}
      aria-label={status}
      title={status}
    >
      <i class={statusTone}></i>
      {#if statusTone !== "gray"}<span>{status}</span>{/if}
    </span>
  </header>

  <div class="bot-card-visual">
    <MinecraftSkin {username} {profileId} size={154} />
  </div>

  <div class="bot-card-content">
    <strong class="bot-card-username" title={username}>{username}</strong>
    <label class="bot-card-server">
      <input
        value={serverAddress}
        aria-label={`${serverAddressLabel}: ${username}`}
        on:change={(event) => onServerChange(event.currentTarget.value)}
      />
    </label>
  </div>
</article>
