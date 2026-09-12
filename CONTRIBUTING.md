# 參與 Botting

歡迎透過 Issue 討論可重現問題、功能需求與架構改善，再以 Pull Request 提交聚焦的改動。大型變更請先說明問題、使用情境、影響模組及資料相容性。

## 開發環境

依 [README](README.md#快速開始) 安裝 Windows 原生建置工具、WebView2、Node 22 LTS、pnpm 11.9.0 與固定的 Rust nightly。

```powershell
pnpm install --frozen-lockfile
pnpm tauri:dev
```

從適當的基底分支建立工作分支，每次 PR 聚焦一項行為或明確相關的整理。避免混入無關排版、依賴升級及重新命名。

## 程式碼規範

- 遵循 [程式碼慣例](docs/code-conventions.md)：Rust 模組／函式用 `snake_case`，TypeScript 工具檔用 `kebab-case`，函式／變數用 `camelCase`，Svelte 元件用 `PascalCase`。
- IPC 和持久化欄位是相容性契約；修改時同步 Rust／TypeScript 與資料遷移說明。保留既有 Registry 路徑，除非一併提供遷移方案。
- UI 元件處理呈現及互動，feature 處理狀態，adapter 處理 IPC，commands 保持薄層。純規則放在不依賴 UI 或連線的模組。
- 關鍵函式補上繁體中文用途、`@param`、`@return`；Rust 使用 `///`，TypeScript 使用 JSDoc。核心邏輯註解解釋狀態來源、時間單位及資源生命週期。
- 每個自有核心原始碼檔保留 `SPDX-License-Identifier: AGPL-3.0-only`、原作者版權及授權 Header。第三方檔案保留其原始聲明。
- 新增依賴需說明用途及授權，保留 `Cargo.lock` 和 `pnpm-lock.yaml`，更新第三方來源紀錄。

## 驗證改動

執行與變更相稱的既有檢查。核心業務變動應執行對應前端或 Rust 測試；純註解及排版變更以格式、型別與編譯檢查為主。

```powershell
pnpm format:check
pnpm check
pnpm test
cargo test --workspace --lib --locked
pnpm lint:rust
pnpm build
git diff --check
```

**不要主動新增不必要的單元測試、Mock 或測試腳本。** 不為可逆的小修改、註解、格式或單純複述實作增加測試。必要的行為回歸驗證應能證明實際風險，並在 PR 說明原因。

現有檢查通過後，只有新改動、新失敗或未解疑點才擴大或重跑。UI 行為變動應檢查明暗主題、視窗縮放與鍵盤操作；涉及登入或遊戲連線時，說明實際驗證環境及未覆蓋路徑。移除本次新增且無後續用途的暫存檔。

## 提交與審查

PR 描述應包含具體問題、最後行為、相關模組及實際跑過的檢查。若變動會影響登入、事件世代、停止等待或儲存格式，說明其相容性及失敗處理。未執行的檢查標記為未執行，不能以靜態閱讀替代實際測試結果。

Issue 請附版本、Windows 版本、功能模式、重現步驟與預期／實際結果。日誌只提供重現所需片段，不提交 Cookie、access token、refresh token、Registry 匯出或帳號憑據。使用既有範本即可，不必另外建立測試專案。

## 授權與來源

提交者須有權以 `AGPL-3.0-only` 提供自己的貢獻，並保留他人的必要聲明。引用第三方程式碼或素材時，附原始網址、作者、版本與授權；不把第三方檔案的 Header 改成自己的版權。

專案使用標準 [AGPLv3 全文](LICENSE)，沒有額外加入「單純閱讀就必須公開所有作品」等條件。散布受涵蓋作品或修改後提供網路互動時，依條款提供完整對應原始碼。

## 發布前檢查

維護者使用已登入的 GitHub CLI 執行：

```powershell
pnpm release:check
```

此命令確認必要文件、來源 Header、npm 防誤發布欄位、已追蹤的本機資料檔名、乾淨工作目錄及 `origin` 的 GitHub 可見性。私有、內部或無法確認可見性的倉庫會被拒絕。GitHub 的 **Public Release Preflight** 工作流程提供相同的手動入口，僅授予 `contents: read`。

兩個 npm package 維持 `private: true`，兩個 Rust crate 設定 `publish = false`。這些設定防止套件管理器誤發布；它們不控制 GitHub 網頁、管理員操作或其他自行新增的發布流程。現有檢查沒有公開倉庫、推送、上傳產物或建立 Release 的動作。

公開散布時，將 LICENSE、必要第三方聲明與該二進位精確對應的原始碼及建置資料一併提供。套件清單與檔名檢查不代表已完成所有歷史版本的憑據或素材權利查核。
