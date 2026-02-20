use crate::utils::bytes_safe_formatting::format_bytes;
use crate::utils::pm3_safe_dir;
use crate::{models::pm3_config::PmProcessConfig, utils::pm3_safe_cfg_handler};
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Pid, System};
use tokio::sync::Mutex;

use std::{fs::File, process::Stdio};
use tokio::process::{Child, Command};

#[derive(Debug)]
pub struct PmProcess {
    pub idx: u64,
    pub config: PmProcessConfig,
    pub handle: Arc<Mutex<Option<Child>>>,
    pub process_status: PmProcessStatus,
}

#[derive(Debug)]
pub enum PmProcessStatus {
    Disabled,
    NotStarted,
    Initializing,
    InitializingFailed,
    Running,
    Stopped,
    Exited(u32),
    Finished,
}

impl PmProcess {
    pub fn new(cfg: PmProcessConfig, idx: u64) -> Self {
        PmProcess {
            idx: idx,
            config: cfg,
            handle: Arc::new(Mutex::new(None)),
            process_status: PmProcessStatus::NotStarted,
        }
    }

    pub async fn awake(&mut self) -> std::io::Result<()> {
        self.process_status = PmProcessStatus::Initializing;

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

        self.process_status = PmProcessStatus::Running;

        Ok(())
    }

    pub async fn monitor(&mut self, sys: &mut System) {
        let (name, handle) = { (self.config.proc_name.clone(), Arc::clone(&self.handle)) };

        let prefix = format!("{name} [process]");

        let (pid_u32, exited) = {
            let mut h = handle.lock().await;

            let Some(child) = h.as_mut() else {
                eprintln!("[pm3] {prefix} no handle");
                return;
            };

            let pid = child.id().unwrap_or(0);

            let exited = match child.try_wait() {
                Ok(Some(status)) => {
                    println!("{prefix} exited: {status}");
                    *h = None;
                    true
                }
                Ok(None) => false,
                Err(e) => {
                    eprintln!("[pm3] {prefix} try_wait error: {e}");
                    false
                }
            };

            (pid, exited)
        };

        if exited {
            return;
        }

        let pid = Pid::from_u32(pid_u32);
        sys.refresh_process(pid);

        if let Some(proc_) = sys.process(pid) {
            let mem_mb = proc_.memory() as f64;
            let cpu = proc_.cpu_usage();
            println!(
                "{prefix} [monitor] CPU: {:.2}% | RAM: {}",
                cpu,
                format_bytes(mem_mb)
            );
        } else {
            println!("{prefix} process not found (pid={})", pid.as_u32());
        }
    }

    pub fn dump_config(&self) -> anyhow::Result<PathBuf> {
        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let config_file_path = pm3_home_dir
            .join("configs")
            .join(format!("{}.proc", self.config.proc_name));

        pm3_safe_cfg_handler::save_config(&self.config, &config_file_path)?;

        Ok(config_file_path)
    }
}
