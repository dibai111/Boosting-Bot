use crate::app_state::CommandResult;
use chrono::Utc;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};

const FILE_DROP_GRANT_TTL: Duration = Duration::from_secs(5);

struct FileDropGrant {
    window_label: String,
    path: PathBuf,
    expires_at: Instant,
}

#[derive(Default)]
pub(crate) struct DroppedFileAccess {
    grant: Mutex<Option<FileDropGrant>>,
}

impl DroppedFileAccess {
    pub(crate) fn authorize_native_drop(&self, window_label: &str, paths: &[PathBuf]) {
        let canonical_path = paths
            .first()
            .and_then(|path| std::fs::canonicalize(path).ok());
        self.replace_grant(window_label, canonical_path, Instant::now());
    }

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
    let filename = format!(
        "botting-session-{}.log",
        Utc::now().format("%Y-%m-%dT%H-%M-%S")
    );
    let path = directory.join(filename);
    std::fs::write(&path, contents).map_err(|error| format!("write session log: {error}"))?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub(crate) fn default_export_log_path(app: tauri::AppHandle) -> CommandResult<String> {
    app.path()
        .download_dir()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| format!("find Downloads folder: {error}"))
}

#[tauri::command]
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
