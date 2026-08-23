use anyhow::{Context, Result};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::{self, Duration, MissedTickBehavior},
};

const POLL_INTERVAL: Duration = Duration::from_millis(300);
const SIGNATURE_BYTES: u64 = 256;
const SIGNATURE_CHECK_INTERVAL: Duration = Duration::from_secs(2);
const MAX_CHUNK_BYTES: u64 = 256 * 1024;
const MAX_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub enum PlayerLogMessage {
    Line(String),
    Unavailable(String),
}

pub struct PlayerLogTailer {
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

struct ReadChunk {
    bytes: Vec<u8>,
    reset: bool,
}

impl PlayerLogTailer {
    pub async fn start(path: PathBuf, messages: mpsc::Sender<PlayerLogMessage>) -> Result<Self> {
        let metadata = tokio::fs::metadata(&path)
            .await
            .with_context(|| format!("read player log {}", path.display()))?;
        if !metadata.is_file() {
            anyhow::bail!("player log path is not a file");
        }

        let (stop, stop_receiver) = oneshot::channel();
        let task = tokio::spawn(run(path, metadata.len(), messages, stop_receiver));
        Ok(Self {
            stop: Some(stop),
            task,
        })
    }

    pub async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = self.task.await;
    }
}

async fn run(
    path: PathBuf,
    mut offset: u64,
    messages: mpsc::Sender<PlayerLogMessage>,
    mut stop: oneshot::Receiver<()>,
) {
    let mut signature = None;
    let mut signature_checked_at = Instant::now();
    let mut partial = Vec::new();
    let mut unavailable = false;
    let mut interval = time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = &mut stop => break,
            _ = interval.tick() => {
                match read_new_bytes(
                    &path,
                    &mut offset,
                    &mut signature,
                    &mut signature_checked_at,
                ).await {
                    Ok(chunk) => {
                        unavailable = false;
                        if chunk.reset {
                            // Log 輪替後要丟棄舊檔案未完成的半行，避免與新檔案拼接。
                            partial.clear();
                        }
                        if !chunk.bytes.is_empty()
                            && emit_lines(&mut partial, &chunk.bytes, &messages, &mut stop)
                                .await
                                .is_err()
                        {
                            break;
                        }
                    }
                    Err(error) if !unavailable => {
                        unavailable = true;
                        if send_message(
                            &messages,
                            &mut stop,
                            PlayerLogMessage::Unavailable(error.to_string()),
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }
}

async fn send_message(
    messages: &mpsc::Sender<PlayerLogMessage>,
    stop: &mut oneshot::Receiver<()>,
    message: PlayerLogMessage,
) -> Result<(), ()> {
    tokio::select! {
        _ = &mut *stop => Err(()),
        result = messages.send(message) => result.map_err(|_| ()),
    }
}

async fn read_new_bytes(
    path: &Path,
    offset: &mut u64,
    signature: &mut Option<u64>,
    signature_checked_at: &mut Instant,
) -> Result<ReadChunk> {
    let metadata = tokio::fs::metadata(path).await?;
    let mut reset = metadata.len() < *offset;
    if reset {
        *offset = 0;
    }

    // 閒置輪詢時不必每次開檔及雜湊；定期檢查仍可偵測 Log 輪替，亦毋須加入 watcher 依賴。
    if signature.is_none() || signature_checked_at.elapsed() >= SIGNATURE_CHECK_INTERVAL {
        let current_signature = file_signature(path).await?;
        let signature_changed = !reset
            && *offset >= SIGNATURE_BYTES
            && signature.is_some_and(|value| value != current_signature);
        reset |= signature_changed;
        if reset {
            *offset = 0;
        }
        *signature = Some(current_signature);
        *signature_checked_at = Instant::now();
    }

    if metadata.len() <= *offset {
        return Ok(ReadChunk {
            bytes: Vec::new(),
            reset,
        });
    }

    let mut file = File::open(path).await?;
    file.seek(SeekFrom::Start(*offset)).await?;
    let mut bytes = Vec::new();
    file.take(MAX_CHUNK_BYTES).read_to_end(&mut bytes).await?;
    *offset += bytes.len() as u64;
    Ok(ReadChunk { bytes, reset })
}

async fn file_signature(path: &Path) -> Result<u64> {
    let mut file = File::open(path).await?;
    let mut bytes = [0u8; SIGNATURE_BYTES as usize];
    let length = file.read(&mut bytes).await?;
    let mut hasher = DefaultHasher::new();
    bytes[..length].hash(&mut hasher);
    Ok(hasher.finish())
}

async fn emit_lines(
    partial: &mut Vec<u8>,
    bytes: &[u8],
    messages: &mpsc::Sender<PlayerLogMessage>,
    stop: &mut oneshot::Receiver<()>,
) -> Result<(), ()> {
    partial.extend_from_slice(bytes);
    let mut consumed = 0;
    while let Some(relative_newline) = partial[consumed..].iter().position(|byte| *byte == b'\n') {
        let newline = consumed + relative_newline;
        let mut line_end = newline;
        if line_end > consumed && partial[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        if line_end - consumed <= MAX_LINE_BYTES {
            let text = decode_log_line(&partial[consumed..line_end]);
            send_message(messages, stop, PlayerLogMessage::Line(text)).await?;
        }
        consumed = newline + 1;
    }

    if consumed != 0 {
        let remaining = partial.len() - consumed;
        partial.copy_within(consumed.., 0);
        partial.truncate(remaining);
    }
    if partial.len() > MAX_LINE_BYTES {
        partial.clear();
    }
    Ok(())
}

fn decode_log_line(bytes: &[u8]) -> String {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes.get(index..index + 2) == Some(&[0xA1, 0xEC])
            && bytes.get(index + 2).is_some_and(u8::is_ascii)
        {
            index += 3;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&normalized).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{decode_log_line, emit_lines, PlayerLogMessage};
    use tokio::sync::{mpsc, oneshot};

    #[test]
    fn removes_cp950_minecraft_formatting_codes() {
        let line = b"[CHAT] \xA1\xECb[MVP\xA1\xEC2+\xA1\xECb] Prank_Monkey6969\xA1\xECf: hi";
        assert_eq!(decode_log_line(line), "[CHAT] [MVP+] Prank_Monkey6969: hi");
    }

    #[tokio::test]
    async fn stop_signal_interrupts_message_send_when_channel_is_full() {
        let (messages, _receiver) = mpsc::channel(1);
        messages
            .send(PlayerLogMessage::Line("queued".to_owned()))
            .await
            .expect("channel should accept the first message");
        let (stop, mut stop_receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut partial = Vec::new();
            emit_lines(&mut partial, b"blocked\n", &messages, &mut stop_receiver).await
        });

        stop.send(()).expect("stop receiver should still be active");

        assert!(task.await.expect("message task should finish").is_err());
    }
}
