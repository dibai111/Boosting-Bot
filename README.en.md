<div align="center">
  <img src="docs/assets/botting-icon.png" width="96" alt="Botting icon">
  <h1>Botting</h1>
  <p>A lightweight desktop tool for managing Minecraft bot sessions.</p>
  <p>
    <a href="README.md">繁體中文</a>
    ·
    <a href="README.en.md">English</a>
  </p>
</div>

## Overview

Botting brings Minecraft account management, bot lifecycle control, server targets, matchmaking, and session logs into one Windows desktop interface. It is designed for managing multiple bots and monitoring their live state from one place.

## Features

- **Account management**: Add Minecraft accounts with Microsoft login, access tokens, or cookies.
- **Bot control**: Start or stop bots individually or in batches, with an independent server target for each account.
- **Matchmaking sessions**: Support BedWars, Duels, and SkyWars workflows.
- **Nick Roller**: Filter and apply Hypixel nicknames in a standalone mode; Matching is locked while it runs.
- **Live logs**: Monitor bot state, chat, matchmaking progress, and session logs.
- **Desktop controls**: Use shortcuts, a floating matchmaking overlay, theme switching, and log export.

## Technology Overview

| Area | Technology | Purpose |
| --- | --- | --- |
| UI | Svelte 5, TypeScript, Vite | Build the desktop interface, components, and frontend state flows |
| Desktop | Tauri 2 | Combine the web UI and Rust backend into a Windows application |
| Bot runtime | Rust, Azalea, Tokio, Bevy ECS | Manage Minecraft bot connections, events, and sessions |
| Matchmaking | Rust modules, serde, serde_json | Parse game logs, plan matchmaking flows, and model state |
| Network | reqwest, rustls | Handle HTTP communication for login and game services |
| Local storage | `local-store` crate, Windows APIs | Persist accounts, settings, and local state |
| UI utilities | `@lucide/svelte`, `skinview3d` | Provide icons, Minecraft character views, and UI interactions |

## Source Structure

```text
Botting/
├─ apps/
│  ├─ shared-ui/          Shared Svelte components, motion, and window helpers
│  └─ user-desktop/
│     ├─ src/              Svelte pages, components, adapters, and feature logic
│     └─ src-tauri/src/    Rust commands, bot runtime, auth, and matchmaking
├─ crates/
│  └─ local-store/         Local account, settings, and state storage
├─ docs/assets/            README and application image assets
├─ Cargo.toml              Rust workspace configuration
└─ package.json            pnpm command entry point
```

## Getting Started

Requirements: Windows, Node.js, pnpm, a Rust toolchain, WebView2, and the Windows development tools required by Tauri.

```powershell
cd D:\Codex\Botting
pnpm install
pnpm tauri:dev
```

Frontend checks:

```powershell
pnpm check
pnpm build
```

Rust workspace check:

```powershell
cargo check --workspace
```

Build the desktop package:

```powershell
pnpm tauri:build
```

## Typical Workflow

1. Start Botting and wait for the application to load.
2. Open **Accounts** and add a Microsoft account or import a token/cookie.
3. Configure server targets in **Bots** and select the bots to run.
4. Choose a game mode and matchmaking count in **Matchmaking**.
5. Or select online bots and configure nickname rules in **Nick Roller**; the two operation modes cannot run together.
6. Monitor live state and export logs from **Sessions**.

Only use Minecraft accounts and servers that you are authorized to use, and follow the rules of the relevant services.
