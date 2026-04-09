use crate::logging::logging_service::{IDX_TO_META, LoggingService};
use std::path::PathBuf;

use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

impl LoggingService {
    fn old_log_path(path: &PathBuf) -> PathBuf {
        let parent = path.parent().map(PathBuf::from).unwrap_or_default();
        let file_name = path
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_else(|| "log.log".to_string());

        let old_name = if let Some(stripped) = file_name.strip_suffix(".log") {
            format!("{stripped}.old.log")
        } else {
            format!("{file_name}.old.log")
        };

        parent.join(old_name)
    }

    fn get_max_log_size_bytes(idx: u64) -> Option<u64> {
        let map = IDX_TO_META.get().expect("IDX_TO_META not initialized");
        let map = map.lock().expect("IDX_TO_META poisoned");

        map.get(&idx)
            .and_then(|meta| meta.max_log_size)
            .map(|mb| mb.saturating_mul(1024 * 1024))
    }

    async fn rotate_log_file(
        path: &PathBuf,
        file_slot: &mut Option<tokio::fs::File>,
    ) -> anyhow::Result<()> {
        *file_slot = None;

        let old_path = Self::old_log_path(path);

        if fs::try_exists(&old_path).await.unwrap_or(false) {
            let _ = fs::remove_file(&old_path).await;
        }

        if fs::try_exists(path).await.unwrap_or(false) {
            fs::rename(path, &old_path).await?;
        }

        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        *file_slot = Some(new_file);
        Ok(())
    }

    pub async fn write_with_rotation(
        idx: u64,
        path: &PathBuf,
        file_slot: &mut Option<tokio::fs::File>,
        data: &[u8],
    ) -> anyhow::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let Some(max_bytes) = Self::get_max_log_size_bytes(idx) else {
            if file_slot.is_none() {
                *file_slot = Some(
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .await?,
                );
            }

            if let Some(file) = file_slot.as_mut() {
                file.write_all(data).await?;
            }

            return Ok(());
        };

        let mut offset = 0usize;

        while offset < data.len() {
            if file_slot.is_none() {
                *file_slot = Some(
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                        .await?,
                );
            }

            let current_size = match fs::metadata(path).await {
                Ok(meta) => meta.len(),
                Err(_) => 0,
            };

            if current_size >= max_bytes {
                Self::rotate_log_file(path, file_slot).await?;
                continue;
            }

            let free_space = (max_bytes - current_size) as usize;

            if free_space == 0 {
                Self::rotate_log_file(path, file_slot).await?;
                continue;
            }

            let to_write = free_space.min(data.len() - offset);

            if let Some(file) = file_slot.as_mut() {
                file.write_all(&data[offset..offset + to_write]).await?;
            }

            offset += to_write;
        }

        Ok(())
    }
}
