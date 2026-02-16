use crate::models::pm3_config::PmProcessConfig;
use crate::utils::pm3_safe_dir;
use std::path::PathBuf;
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
        let filename_abs = PathBuf::from(&self.config.exec_name);

        if !filename_abs.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Executable not found",
            ));
        }

        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let logs_dir = pm3_home_dir.join("processes").join(&self.config.proc_name);
        println!("Logs directory: {}", logs_dir.display());

        match tokio::fs::create_dir_all(&logs_dir).await {
            Ok(_) => println!("Directory created successfully"),
            Err(_) => {}
        }

        let stdout_path = logs_dir.join("stdout.log");
        let stderr_path = logs_dir.join("stderr.log");

        let stdout = File::create(&stdout_path)?;
        let stderr = File::create(&stderr_path)?;

        let child = Command::new(&filename_abs)
            .current_dir(&self.config.exec_dir)
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
