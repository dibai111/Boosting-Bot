# 開源前程式碼審查

審查日期：2026-09-12。基準為本次開始時的工作目錄，包含既有未提交修改；本輪沒有撤回或重新歸屬先前修改。歷史紀錄見 [code-review.md](code-review.md)。

## TODO 進度

- [x] TODO 1：架構、模組責任與命名審查。
- [x] TODO 2：標出維護問題，完成設定解析、候選識別及重複運算整理。
- [x] TODO 3：繁體中文關鍵文檔註解與自有原始碼 Copyleft Header。
- [x] TODO 4：不新增或擴充單元測試、Mock 或測試腳本。
- [x] TODO 5：重寫繁體中文及英文 README。
- [x] TODO 6：完整 AGPLv3 LICENSE、貢獻指南、Issue／PR 範本與私有倉庫發布前檢查。

此清單表示本輪審查與交付項目完成，不代表下列已知缺陷已全部修正，也不表示已公開倉庫。

## 尚未修正的發現

### P1：工作模式啟停不是完整交易

位置：`apps/user-desktop/src-tauri/src/runtime/mod.rs` 的 `enter_mode`／`reconcile_active_mode`，以及 `runtime/nick.rs`、`runtime/matchmaking.rs`。

Nick 啟動先設定模式，再透過另一把鎖填入 Bot 集合並逐一發送指令；配對啟動也在模式更新後才等待協調器啟動。此時若另一個 IPC 呼叫查詢或停止模式，可能看到空集合／Idle 配對階段而提前釋放模式。模式互斥鎖目前只涵蓋欄位更新，沒有涵蓋實際啟停操作。

另外，Failed 配對仍保留計畫與日誌追蹤器，`coordinator/round.rs` 允許 Failed 收到玩家轉服後重啟；應用層卻可能已將模式校正成 Idle。這使其他工作模式有機會與恢復中的配對交錯。需要統一生命週期交易及終止語意，並驗證舊事件、背景重試與日誌消費任務的失效規則。本輪未修改此跨模組流程，未宣稱已透過真實連線重現。

### P1：停止 Bot 沒有總等待上限

位置：`apps/user-desktop/src-tauri/src/bot_runtime/engine.rs` 的 `SessionHandle::stop`，以及 `session.rs` 的連線主迴圈。

停止先等待指令通道發送，再對完成通知等待五秒，最後仍等待 `thread.join()`。指令通道或 Azalea 連線執行緒若無法退出，整個停止要求仍可能長時間不返回，並阻擋後續關閉。新增文檔已準確註明五秒不是總上限；修正需為連線主迴圈設計可取消退出，不能只捨棄 join 而留下活躍連線。

### P2：主頁責任與全域樣式仍集中

位置：`apps/user-desktop/src/App.svelte`、`apps/user-desktop/src/app.css`、`NickRollerPanel.svelte`。

主頁仍同時管理頁面、帳號表單、設定、日誌、快捷鍵與對話框，且保留多個只轉呼叫 controller 的包裝。Nick 設定解析已抽出，但主頁與其他面板仍可依業務拆分。Svelte CSS parser 掃描全域 CSS 得到 995 個規則，149 組相同 at-rule 情境內重複的 selector；例如 `.app-shell` 出現五次。重複 selector 不等於無效 CSS，直接刪除或重排會改變層疊結果，應搭配畫面比對逐區整理。

### P2：素材來源未完整記錄

內附 PNG／ICO／ICNS 圖示缺少獨立作者與來源紀錄。本輪新增 [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) 列出實際路徑與依賴 metadata，沒有替二進位素材杜撰版權來源。完整散布聲明仍需保留實際第三方 LICENSE／NOTICE。

## 本輪修正

| 問題                                                                            | 調整                                                                                            | 影響範圍                                       |
| ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| Nick 設定把任意 JSON 斷言成有效型別，陣列欄位為字串時可能使畫面呼叫 `join` 失敗 | 抽出 `features/nick-roller-config.ts`，逐欄驗證布林、有限數值、陣列與模式；保留 null 的停用語意 | 設定還原與 Nick 表單；有效設定維持原語意       |
| 同一 Bot 重啟 Nick 篩選會將候選編號歸零，舊決策可能與新候選撞號                 | 移除啟動時的計數器重設，編號在同一連線內延續                                                    | Nick 狀態機；尚未處理跨 Bot 重新連線的決策識別 |
| Nick 驗證完成函式仍帶未使用的時間參數                                           | 移除參數並更新兩個呼叫點                                                                        | 私有函式，沒有變更 IPC                         |
| 名稱長度被重複計算，Contains 比對建立暫時 Vec                                   | 長度只計算一次，改用可短路的迭代器                                                              | 保留優先規則與錯誤訊息                         |
| 聊天分類反覆正規化同一文字                                                      | 一次正規化後交給遊戲開始／結束判斷                                                              | 共用 Hypixel 訊號解析                          |
| 前端每次配對建立 `bot_usernames`，後端沒有該欄位且使用儲存名稱                  | 移除冗餘 payload、型別與暫時集合                                                                | 不改變後端實際採用的名稱來源                   |
| Cookie 重新導向只檢查主機，未檢查 scheme 與埠                                   | 只允許既有登入主機的 HTTPS／443 端點                                                            | Cookie 身分交換                                |
| 缺少授權與發布限制                                                              | 加入 AGPLv3 全文、108 個自有原始碼／入口 Header、package 授權 metadata 與套件防誤發布欄位       | 原始碼授權及開源文件                           |

## 架構與命名結論

前端已有 components、features、adapters、models、log 分層，後端已有 commands、runtime、Bot actor、配對 coordinator 與儲存 crate。`local-store` 不依賴桌面 UI；同 crate 中配對與 Bot model 互相引用，整體仍由應用層組裝，沒有為這些內部型別另建空泛抽象。

全域命名以語言慣例為準：Rust 使用 snake_case，Svelte 元件使用 PascalCase，TypeScript 工具使用 kebab-case、函式及變數使用 camelCase。IPC snake_case、`Software\\BoostingBot` 儲存路徑與 `bedwars-boosting-icon.png` 歷史素材名保留，避免無遷移的相容性變更。Rust 模組的 `mod.rs`、crate 的 `lib.rs`／`main.rs` 是工具鏈入口，並非命名不一致。

本輪使用 TypeScript 與 Svelte parser 掃描 34 個前端來源檔，解析相對匯入及既有 API alias，沒有發現無法解析的相對引用或匯入循環。這是依賴結構檢查，不是桌面功能端到端驗證。

## 文件與私有倉庫防護

核心儲存、登入、Bot 生命週期、Nick 狀態機、配對協調器、Tauri 指令與前端 API／feature 已補上繁體中文用途、參數與回傳說明。簡單呈現用回呼保留精簡；標準第三方授權 Header 保留英文。產生的 Tauri schema、套件依賴與二進位檔不插入自有版權註解。

本輪直接從 GNU 官網取得完整 AGPLv3 條款。README 明確區分散布、修改後網路互動及單純使用／閱讀，沒有把標準 AGPL 改寫成要求所有獨立作品公開的額外限制。

`pnpm release:check` 與手動 GitHub workflow 只執行發布前檢查，私有／內部／無法確認可見性的倉庫均拒絕。npm 的 `private: true` 與 Cargo 的 `publish = false` 防止對套件 registry 誤發布；沒有修改 GitHub 可見性、push、上傳產物或建立 Release。管理員手動操作及另外新增的發布流程不受這些本機設定控制。

## 驗證紀錄

| 檢查                                                                  | 本輪結果                                                                                                                        |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm format:check`、`git diff --check`                               | 通過。                                                                                                                          |
| `pnpm check`                                                          | 0 errors、0 warnings。                                                                                                          |
| `pnpm test`                                                           | 11 個既有前端測試通過。                                                                                                         |
| `cargo test --workspace --lib --locked -j 2`                          | 主程式 61 個、本機儲存 3 個測試通過。                                                                                           |
| `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings` | 通過全部 targets。                                                                                                              |
| `pnpm build`                                                          | 正式前端建置成功。主 JS 約 246.58 kB；延遲載入的 skinview3d 分塊約 519.21 kB，仍有超過 500 kB 的提示。                          |
| Nick 設定還原直接呼叫                                                 | 錯誤陣列型別回退為空清單、null 長度維持停用、音量上限 100、書本等待下限 500 ms、損壞 JSON 回退預設值。                          |
| PowerShell parser                                                     | 發布前檢查腳本語法通過。                                                                                                        |
| `pnpm release:check`                                                  | 按預期以 exit code 1 阻擋 PRIVATE 倉庫與未提交工作目錄；必要文件、package guards、已追蹤檔名及 108 個來源 Header 均無額外失敗。 |
| Cargo metadata                                                        | 兩個 crate 均為 AGPL-3.0-only，且允許發布的 registry 清單為空。                                                                 |
| 授權正文與文件連結                                                    | LICENSE 與此次取得的 GNU 官方正文一致（忽略首尾空白）；18 個 Markdown 本機連結均可解析。                                        |

第一次以預設平行數執行 Rust 測試時，第三方 `rustversion` 編譯遇到 Windows 檔案共用衝突 `0xc0000043`；待程序結束後以 `-j 2` 重跑已成功。Node 的型別移除 experimental 提示與 Rust 連結器的 `.lib`／`.exp` 建立訊息沒有造成最終檢查失敗。

未執行真實 Microsoft 登入、Hypixel 連線、Tauri 安裝包或完整桌面互動驗證。本輪沒有以新測試覆蓋上述兩項 P1 競態與停止問題，也沒有宣稱這些路徑已修復。

本輪沒有新增或擴充單元測試、Mock 或測試腳本。一次性 AST／metadata 檢查直接在終端執行，沒有保留暫存檔。`scripts/check-public-release.ps1` 是持續使用的發布前工具，不是測試腳本。
