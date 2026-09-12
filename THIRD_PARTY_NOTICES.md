# 第三方來源與授權

Botting 自有原始碼使用 AGPL-3.0-only。下列依賴與其內容仍適用原始授權，專案 Header 不取代第三方版權。版本取自本次鎖定依賴與套件 metadata；更新依賴時需重新核對。

## 主要執行依賴

| 套件               | 鎖定版本          | 套件宣告授權      | 上游                                                                        |
| ------------------ | ----------------- | ----------------- | --------------------------------------------------------------------------- |
| Svelte             | 5.56.7            | MIT               | [sveltejs/svelte](https://github.com/sveltejs/svelte)                       |
| @tauri-apps/api    | 2.11.1            | Apache-2.0 OR MIT | [tauri-apps/tauri](https://github.com/tauri-apps/tauri)                     |
| @lucide/svelte     | 1.25.0            | ISC               | [lucide-icons/lucide](https://github.com/lucide-icons/lucide)               |
| skinview3d         | 3.4.2             | MIT               | [bs-community/skinview3d](https://github.com/bs-community/skinview3d)       |
| Azalea             | 0.16.0+mc26.1     | MIT               | [azalea-rs/azalea](https://github.com/azalea-rs/azalea)                     |
| Bevy ECS           | 0.18.1            | MIT OR Apache-2.0 | [bevyengine/bevy](https://github.com/bevyengine/bevy)                       |
| Tauri              | 2.11.5            | Apache-2.0 OR MIT | [tauri-apps/tauri](https://github.com/tauri-apps/tauri)                     |
| Tokio              | 1.53.1            | MIT               | [tokio-rs/tokio](https://github.com/tokio-rs/tokio)                         |
| reqwest            | 0.13.2            | MIT OR Apache-2.0 | [seanmonstar/reqwest](https://github.com/seanmonstar/reqwest)               |
| serde / serde_json | 1.0.229 / 1.0.151 | MIT OR Apache-2.0 | [serde-rs](https://github.com/serde-rs)                                     |
| anyhow             | 1.0.104           | MIT OR Apache-2.0 | [dtolnay/anyhow](https://github.com/dtolnay/anyhow)                         |
| base64             | 0.22.1            | MIT OR Apache-2.0 | [marshallpierce/rust-base64](https://github.com/marshallpierce/rust-base64) |
| chrono             | 0.4.45            | MIT OR Apache-2.0 | [chronotope/chrono](https://github.com/chronotope/chrono)                   |
| rand               | 0.8.7             | MIT OR Apache-2.0 | [rust-random/rand](https://github.com/rust-random/rand)                     |
| uuid               | 1.24.0            | Apache-2.0 OR MIT | [uuid-rs/uuid](https://github.com/uuid-rs/uuid)                             |
| windows-sys        | 0.61.2            | MIT OR Apache-2.0 | [microsoft/windows-rs](https://github.com/microsoft/windows-rs)             |

這是主要直接依賴索引，不是完整二進位散布聲明。完整解析依賴以 `Cargo.lock`、`pnpm-lock.yaml` 及安裝套件中的 LICENSE／NOTICE 為準，包含 Three.js、TLS、WebView 及各自的間接依賴。Lucide 套件內可能另有繼承自上游圖示的聲明，應保留套件提供的完整檔案。

## 素材與外部服務

倉庫內圖示位於 `docs/assets/botting-icon.png`、`apps/user-desktop/public/bedwars-boosting-icon.png` 與 `apps/user-desktop/src-tauri/icons/`。現有倉庫未提供獨立的素材作者與來源紀錄，本次未替這些二進位素材杜撰來源或另宣稱可重新授權；維護者應補齊實際作者與再散布依據。

玩家頭像與皮膚於執行時從 `mc-heads.net`、`visage.surgeplay.com` 載入，並非本專案授權的內附素材。Microsoft、Minecraft、Xbox、Hypixel 名稱與服務標識屬各自權利人；本專案 AGPL 授權不授予這些商標權。

## 散布時

保留實際隨程式散布的第三方 LICENSE／NOTICE，依所選授權遵守條件，並提供 AGPL 所要求的本版本完整對應原始碼及建置資料。此索引不替代各上游的完整授權條款，也未聲稱素材來源已完成查核。
