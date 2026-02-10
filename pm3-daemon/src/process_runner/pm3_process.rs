use crate::models::pm3_config::PmProcessConfig;
use crate::utils::pm3_safe_dir;
use std::sync::Arc;
use tokio::sync::Mutex;

use std::{fs::File, process::Stdio};
use tokio::process::{Child, Command};

#[derive(Debug)]
pub struct PmProcess {
    pub config: PmProcessConfig,
    pub handle: Arc<Mutex<Option<Child>>>,
}

impl PmProcess {
    pub fn new(cfg: PmProcessConfig) -> Self {
        PmProcess {
            config: cfg,
            handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn awake(&mut self) -> std::io::Result<()> {
        let proc_name = self.config.exec_name.as_str();

        let filename_abs = self
            .config
            .exec_dir_absolute_path
            .join(&self.config.exec_name);

        if !filename_abs.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Executable not found",
            ));
        }

        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let logs_dir = pm3_home_dir.join("processes").join(proc_name);
        tokio::fs::create_dir_all(&logs_dir).await?;

        let stdout_path = logs_dir.join("stdout.log");
        let stderr_path = logs_dir.join("stderr.log");

        let stdout = File::create(&stdout_path)?;
        let stderr = File::create(&stderr_path)?;

        let child = Command::new(&filename_abs)
            .args(&self.config.exec_args)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;

        {
            let mut guard = self.handle.lock().await;
            *guard = Some(child);
        }

        println!("Process: {:?}", self);

        Ok(())
    }
}
