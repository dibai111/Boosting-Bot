<script lang="ts">
  import MinecraftSkin from "../minecraft/MinecraftSkin.svelte";
  import HexagonPattern from "../../../../../shared-ui/HexagonPattern.svelte";

  export let username = "";
  export let profileId = "";
  export let usernameLabel = "Username";
  export let loginMethod = "";
  export let loginMethodLabel = "Login method";
  export let status = "";
  export let statusTone = "gray";
  export let selected = false;
</script>

<button
  type="button"
  class="account-card"
  class:selected={selected}
  aria-pressed={selected}
  aria-label={`${usernameLabel}: ${username}`}
  on:click
>
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
  <span class="account-card-status status-dot-only" aria-label={status} title={status}>
    <i class={statusTone}></i>
  </span>
  <div class="account-card-visual">
    <MinecraftSkin username={username} profileId={profileId} size={154} />
  </div>
  <div class="account-card-content">
    <div class="account-card-meta">
      <span><small>{usernameLabel}</small><b title={username}>{username}</b></span>
      <span><small>{loginMethodLabel}</small><b>{loginMethod}</b></span>
    </div>
  </div>
</button>
