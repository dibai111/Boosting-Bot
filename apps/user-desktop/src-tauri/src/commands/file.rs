/*
 * SPDX-License-Identifier: AGPL-3.0-only
 * Copyright (C) 2026 baibai and Botting contributors
 *
 * Botting is free software: you can redistribute it and/or modify it under
 * the GNU Affero General Public License version 3, as published by the
 * Free Software Foundation. This program comes WITHOUT ANY WARRANTY;
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
 * PARTICULAR PURPOSE. See the LICENSE file for the complete terms.
 * Copyleft: covered modifications must retain these license obligations.
 * https://www.gnu.org/licenses/agpl-3.0.html
 */

//! 處理登入連結、記錄匯出及原生拖放檔案的單次讀取權限。

use crate::app_state::CommandResult;
use chrono::Utc;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};
use uuid::Uuid;

const FILE_DROP_GRANT_TTL: Duration = Duration::from_secs(5);

struct FileDropGrant {
    window_label: String,
    path: PathBuf,
    expires_at: Instant,
}

#[derive(Default)]
/// 持有最近一次原生拖放的短效、單次檔案讀取授權。
pub(crate) struct DroppedFileAccess {
    grant: Mutex<Option<FileDropGrant>>,
}

impl DroppedFileAccess {
    /// 以原生拖放的第一個檔案替換舊授權。
    /// @param window_label 產生拖放的視窗標籤。
    /// @param paths Tauri 原生拖放提供的路徑清單。
    /// @return 無回傳值；無法正規化時清除授權。
    pub(crate) fn authorize_native_drop(&self, window_label: &str, paths: &[PathBuf]) {
        let canonical_path = paths
            .first()
            .and_then(|path| std::fs::canonicalize(path).ok());
        self.replace_grant(window_label, canonical_path, Instant::now());
    }

    /// 核對視窗、路徑及期限，消耗最近的讀取授權。
    /// @param window_label 要求讀取的視窗標籤。
    /// @param candidate 前端傳入的候選路徑。
    /// @return 授權的正規路徑；不符合條件時為 None。
    pub(crate) fn take_authorized_path(
        &self,
        window_label: &str,
        candidate: &Path,
    ) -> Option<PathBuf> {
        let canonical_candidate = std::fs::canonicalize(candidate).ok()?;
        self.take_canonical_path(window_label, &canonical_candidate, Instant::now())
    }

    fn replace_grant(&self, window_label: &str, path: Option<PathBuf>, now: Instant) {
        // 原生 drop 只提供短效、單次能力，避免 WebView 將 command 當成任意檔案讀取器。
        let grant = path.map(|path| FileDropGrant {
            window_label: window_label.to_owned(),
            path,
            expires_at: now + FILE_DROP_GRANT_TTL,
        });
        *self.grant.lock().expect("file drop grant lock") = grant;
    }

    fn take_canonical_path(
        &self,
        window_label: &str,
        candidate: &Path,
        now: Instant,
    ) -> Option<PathBuf> {
        let grant = self.grant.lock().expect("file drop grant lock").take()?;
        if grant.window_label != window_label
            || now > grant.expires_at
            || grant.path.as_path() != candidate
        {
            return None;
        }
        Some(candidate.to_owned())
    }
}

#[tauri::command]
/// 只允許開啟 Microsoft HTTPS 登入網址。
/// @param url 裝置代碼登入的驗證網址。
/// @return 系統啟動瀏覽器結果；非 Windows 不支援。
pub(crate) fn open_external_url(url: String) -> CommandResult<()> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "invalid sign-in URL".to_owned())?;
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https" || !(host == "microsoft.com" || host.ends_with(".microsoft.com"))
    {
        return Err("sign-in URL must use a Microsoft HTTPS domain".to_owned());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(parsed.as_str())
            .spawn()
            .map_err(|error| format!("open sign-in page: {error}"))?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    Err("opening the sign-in page is only supported on Windows".to_owned())
}

#[tauri::command]
/// 在指定目錄建立唯一檔名，避免覆寫已匯出的日誌。
/// @param contents 目前篩選後的日誌文字。
/// @param folder 絕對目錄；空白時使用 Downloads。
/// @param app 目前 Tauri 應用控制柄。
/// @return 匯出檔案的完整路徑或 I/O 錯誤。
pub(crate) fn export_session_log(
    contents: String,
    folder: String,
    app: tauri::AppHandle,
) -> CommandResult<String> {
    let directory = if folder.trim().is_empty() {
        app.path()
            .download_dir()
            .map_err(|error| format!("find Downloads folder: {error}"))?
    } else {
        let directory = PathBuf::from(folder.trim());
        if !directory.is_absolute() {
            return Err("export log path must be absolute".to_owned());
        }
        directory
    };

    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("create export log folder: {error}"))?;
    write_session_log(&directory, &contents).map(|path| path.to_string_lossy().into_owned())
}

/// 以 create_new 建立含時間與 UUID 的日誌檔。
/// @param directory 已存在的輸出目錄。
/// @param contents UTF-8 日誌內容。
/// @return 新檔案路徑或寫入錯誤。
fn write_session_log(directory: &Path, contents: &str) -> CommandResult<PathBuf> {
    // 同一秒可連續匯出；唯一名稱配合 create_new，保證不覆寫先前記錄。
    let filename = format!(
        "botting-session-{}-{}.log",
        Utc::now().format("%Y-%m-%dT%H-%M-%S"),
        Uuid::new_v4()
    );
    let path = directory.join(filename);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| format!("create session log: {error}"))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| format!("write session log: {error}"))?;
    Ok(path)
}

#[tauri::command]
/// 取得目前使用者的 Downloads 目錄。
/// @param app 目前 Tauri 應用控制柄。
/// @return 完整目錄路徑。
pub(crate) fn default_export_log_path(app: tauri::AppHandle) -> CommandResult<String> {
    app.path()
        .download_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| format!("find Downloads folder: {error}"))
}

#[tauri::command]
/// 只讀取最近一次授權拖放且不超過大小限制的文字檔。
/// @param path 待讀取的絕對路徑。
/// @param window 提出要求的 Tauri 視窗。
/// @param dropped_files 單次原生拖放授權服務。
/// @return UTF-8 Cookie 文字或授權／I/O 錯誤。
pub(crate) fn read_cookie_file(
    path: String,
    window: tauri::Window,
    dropped_files: State<'_, DroppedFileAccess>,
) -> CommandResult<String> {
    const MAX_COOKIE_FILE_SIZE: u64 = 2 * 1024 * 1024;

    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("cookie file path must be absolute".to_owned());
    }
    let path = dropped_files
        .take_authorized_path(window.label(), &path)
        .ok_or_else(|| "cookie file was not authorized by the latest file drop".to_owned())?;
    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("read cookie file: {error}"))?;
    if !metadata.is_file() {
        return Err("dropped path is not a file".to_owned());
    }
    if metadata.len() > MAX_COOKIE_FILE_SIZE {
        return Err("cookie file is larger than 2 MB".to_owned());
    }
    std::fs::read_to_string(path).map_err(|error| format!("read cookie file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_exports_preserve_both_logs() {
        struct ExportDirectory(PathBuf);
        impl Drop for ExportDirectory {
            fn drop(&mut self) {
                // 僅刪除本測試建立的檔案與空目錄，不使用遞迴刪除。
                if let Ok(entries) = std::fs::read_dir(&self.0) {
                    for entry in entries.flatten() {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
                let _ = std::fs::remove_dir(&self.0);
            }
        }
        let directory = ExportDirectory(
            std::env::temp_dir().join(format!("botting-export-test-{}", Uuid::new_v4())),
        );
        std::fs::create_dir(&directory.0).unwrap();
        let first = write_session_log(&directory.0, "first").unwrap();
        let second = write_session_log(&directory.0, "second").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(first).unwrap(), "first");
        assert_eq!(std::fs::read_to_string(second).unwrap(), "second");
    }

    #[test]
    fn accepts_only_the_latest_drop_and_consumes_it_once() {
        let access = DroppedFileAccess::default();
        let first = PathBuf::from(r"C:\cookie-first.txt");
        let latest = PathBuf::from(r"C:\cookie-latest.txt");
        let now = Instant::now();

        access.replace_grant("main", Some(first.clone()), now);
        access.replace_grant("main", Some(latest.clone()), now);

        assert_eq!(access.take_canonical_path("main", &first, now), None);
        access.replace_grant("main", Some(latest.clone()), now);
        assert_eq!(
            access.take_canonical_path("main", &latest, now),
            Some(latest.clone())
        );
        assert_eq!(access.take_canonical_path("main", &latest, now), None);
    }

    #[test]
    fn rejects_a_grant_for_another_window_or_after_expiry() {
        let access = DroppedFileAccess::default();
        let path = PathBuf::from(r"C:\cookie.txt");
        let now = Instant::now();

        access.replace_grant("main", Some(path.clone()), now);
        assert_eq!(
            access.take_canonical_path("matchmaking-overlay", &path, now),
            None
        );

        access.replace_grant("main", Some(path.clone()), now);
        assert_eq!(
            access.take_canonical_path(
                "main",
                &path,
                now + FILE_DROP_GRANT_TTL + Duration::from_millis(1)
            ),
            None
        );
    }
}
