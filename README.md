<div align="center">
  <img src="docs/assets/botting-icon.png" width="96" alt="Botting icon">
  <h1>Botting</h1>
  <p>一個以桌面介面管理 Minecraft bot 工作階段的輕量工具。</p>
  <p>
    <a href="README.md">繁體中文</a>
    ·
    <a href="README.en.md">English</a>
  </p>
</div>

## 簡介

Botting 將 Minecraft 帳號、bot 啟停、伺服器地址、配對流程和工作階段記錄集中在同一個 Windows 桌面介面中，方便管理多個 bot 和查看即時狀態。

## 功能

- **帳號管理**：使用 Microsoft、Access Token 或 Cookie 新增 Minecraft 帳號。
- **Bot 控制**：逐個或批量啟動、停止 bot，並為每個帳號保存獨立伺服器地址。
- **配對工作階段**：支援 BedWars、Duels 和 SkyWars 的配對流程。
- **Nick Roller**：獨立篩選和套用 Hypixel nick；運行期間會鎖定配對功能。
- **即時記錄**：查看 bot 狀態、聊天記錄、配對進度和 session log。
- **桌面操作**：快捷鍵、浮動配對 overlay、主題切換和記錄匯出。

## 技術概覽

| 區域 | 技術 | 主要用途 |
| --- | --- | --- |
| UI | Svelte 5、TypeScript、Vite | 建立桌面介面、元件和前端狀態流程 |
| Desktop | Tauri 2 | 將 Web UI 與 Rust backend 組合成 Windows 應用程式 |
| Bot runtime | Rust、Azalea、Tokio、Bevy ECS | 管理 Minecraft bot 連線、事件和工作階段 |
| Matchmaking | Rust modules、serde、serde_json | 解析遊戲記錄、規劃配對流程和保存模型 |
| Network | reqwest、rustls | 處理登入及遊戲服務的 HTTP 通訊 |
| Local storage | `local-store` crate、Windows APIs | 保存帳號、設定和本機狀態 |
| UI utilities | `@lucide/svelte`、`skinview3d` | 圖示、Minecraft 角色和介面互動效果 |

## Source 結構

```text
Botting/
├─ apps/
│  ├─ shared-ui/          共用 Svelte 元件、動畫和視窗工具
│  └─ user-desktop/
│     ├─ src/              Svelte 頁面、元件、adapter 和 feature logic
│     └─ src-tauri/src/    Rust commands、bot runtime、auth 和 matchmaking
├─ crates/
│  └─ local-store/         本地帳號、設定和狀態儲存
├─ docs/assets/            README 和應用程式圖片資產
├─ Cargo.toml              Rust workspace 設定
└─ package.json            pnpm 指令入口
```

## 開始使用

需求：Windows、Node.js、pnpm、Rust toolchain，以及 Tauri 所需的 WebView2／Windows 開發工具。

```powershell
cd D:\Codex\Botting
pnpm install
pnpm tauri:dev
```

只檢查前端：

```powershell
pnpm check
pnpm build
```

檢查 Rust workspace：

```powershell
cargo check --workspace
```

建立 desktop package：

```powershell
pnpm tauri:build
```

## 使用流程

1. 啟動 Botting，等待啟動 animation 完成。
2. 開啟 **Accounts**，新增 Microsoft 帳號或匯入 token／cookie。
3. 在 **Bots** 設定各帳號的伺服器地址，選擇要啟動的 bot。
4. 在 **Matchmaking** 選擇遊戲模式和配對數量。
5. 或在 **Nick Roller** 選擇在線 bot、設定 nick 規則，再開始篩選；兩種工作模式不能同時運行。
6. 在 **Sessions** 查看即時狀態和匯出記錄。

請只使用你有權使用的 Minecraft 帳號和伺服器，並遵守相關服務的規則。
