# 改善 TODO

## P0：資料安全與 API 正確性

- [ ] 將對外 `AccountRecord` 與含 credential/session token 的內部 `StoredAccount` 完全分離。
- [ ] 更新 API 對不存在帳號回傳明確錯誤。
- [ ] 統一 profile UUID 驗證、正規化與重複檢查。
- [ ] 統一帳號輸入的非空與長度驗證。

## P1：Windows 11 與儲存層

- [ ] 釐清 Store 的讀改寫併發保證。
- [ ] 以 RAII 管理 Windows DPAPI LocalFree。
- [x] 驗證 Windows 11 target、Registry、DPAPI、Tauri WebView2 與 Windows hook。
- [ ] 補充 Windows-only 回歸測試。

## P2：程式碼品質

- [ ] 統一 Rustdoc 與繁體中文核心註解。
- [ ] 降低 AuthKind mapping 重複。
- [ ] 拆分大型 App.svelte、NickRollerPanel.svelte 與 Rust runtime 檔案。

## 驗收

- [x] pnpm format:check
- [x] pnpm check
- [x] pnpm test
- [x] cargo fmt --all -- --check
- [x] cargo test --workspace --lib --locked
- [x] cargo clippy --workspace --all-targets -- -D warnings
- [x] pnpm build
- [x] git diff --check
- [ ] 建立 commit 並 push 至 GitHub origin
