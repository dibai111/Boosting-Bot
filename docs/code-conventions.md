# 程式碼慣例

本專案使用 Svelte／TypeScript 前端、Tauri／Rust 後端，以及獨立的本機儲存 crate。

## 命名與格式

- Svelte 元件使用 `PascalCase.svelte`；TypeScript 工具檔與資料夾使用 `kebab-case`。
- TypeScript 函式、變數使用 `camelCase`，型別使用 `PascalCase`；布林值以 `is`、`has`、`can`、`should` 等能表達判斷的字詞命名。
- Rust 檔案、模組、函式使用 `snake_case`，型別使用 `PascalCase`，常數使用 `SCREAMING_SNAKE_CASE`。
- IPC／持久化資料欄位沿用現有 `snake_case` 契約；不要只為前端命名偏好更改序列化格式。
- 前端交由 Prettier 格式化，Rust 交由 rustfmt 格式化。使用 UTF-8、LF 換行；前端縮排兩格，Rust 縮排四格。
- 中文註解說明模組責任、狀態轉移、資源生命週期及不直觀的限制，不逐行翻譯程式碼。`SAFETY:` 保留標識，說明以中文撰寫。

## 模組責任

自有核心原始碼檔案保留 `AGPL-3.0-only` SPDX Header 及原作者版權。Rust 以 `///`、TypeScript 以 JSDoc 說明關鍵函式用途、`@param` 與 `@return`；參數需交代時間／角度／尺寸單位、所有權及可空值語意。型別說明描述其責任；無參數函式不加入虛構參數。JSON 與產生的 schema 不插入註解，授權由 package metadata 及 LICENSE 表達。

| 位置                                   | 責任                                           |
| -------------------------------------- | ---------------------------------------------- |
| `apps/shared-ui`                       | 不依賴業務資料的動畫、背景及視窗尺寸工具       |
| `apps/user-desktop/src/lib/components` | 按功能分組的畫面元件                           |
| `apps/user-desktop/src/lib/features`   | 前端狀態與操作流程；可獨立測試的事件轉換       |
| `apps/user-desktop/src/lib/adapters`   | Tauri IPC 與原生視窗介面                       |
| `apps/user-desktop/src/lib/models`     | 前後端資料契約                                 |
| `src-tauri/src/commands`               | 薄層 Tauri 命令入口                            |
| `src-tauri/src/runtime`                | 協調帳號、工作模式與命令生命週期               |
| `src-tauri/src/bot_runtime`            | 單一 Bot 的連線、actor、配對控制及 Nick 狀態機 |
| `src-tauri/src/matchmaking`            | 多 Bot 配對計畫、偵測、重試與提交結果          |
| `crates/local-store`                   | 本機資料儲存與平台保護                         |

新增邏輯放在負責該行為的模組。純規則不要依賴 UI 或連線；避免只有轉呼叫、沒有提供邊界或語意的包裝函式。對外 API 可以保留薄層入口。

## Svelte 與資源生命週期

Svelte 的 `$:` 依賴必須在表達式中可見。函式若讀取事件、階段或帳號，將資料明確傳入，避免函式閉包內的變數更新後畫面沒有重算。

`onMount` 同步回傳清理函式；非同步註冊的監聽使用 `createListenerScope` 追蹤。計時器、音訊及 WebGL 資源也要在卸載時釋放。事件重播不能重設原有的相對期限。

## 檢查指令

```sh
pnpm format
pnpm format:check
pnpm check
pnpm test
pnpm build
cargo test --workspace --lib
pnpm lint:rust
```

前端測試使用 Node 內建測試器與 TypeScript 型別移除，需要支援 `--experimental-strip-types` 的 Node 版本。只為實際行為、邊界條件及曾出現的錯誤增加測試；註解與單純格式調整不需要另寫測試。
