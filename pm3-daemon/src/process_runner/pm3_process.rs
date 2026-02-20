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
    pub proc_name: Arc<str>,
    inner: Mutex<PmProcessInner>,
}

#[derive(Debug)]
struct PmProcessInner {
    config: PmProcessConfig,
    handle: Option<Child>,
    process_status: PmProcessStatus,
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

impl PmProcessStatus {
    pub fn is_active(&self) -> bool {
        match self {
            PmProcessStatus::NotStarted => true,
            PmProcessStatus::Initializing => true,
            PmProcessStatus::Running => true,
            _ => false,
        }
    }
}

impl PmProcess {
    pub fn new(cfg: PmProcessConfig, idx: u64) -> Self {
        let starting_status = match cfg.active {
            true => PmProcessStatus::NotStarted,
            false => PmProcessStatus::Disabled,
        };

        let proc_name: Arc<str> = cfg.proc_name.clone().into();

        PmProcess {
            idx,
            proc_name,
            inner: Mutex::new(PmProcessInner {
                config: cfg,
                handle: None,
                process_status: starting_status,
            }),
        }
    }

    pub async fn awake(&self) -> anyhow::Result<()> {
        self.set_status(PmProcessStatus::Initializing).await;
        let cfg = self.inner.lock().await.config.clone();

        let filename_abs = PathBuf::from(&cfg.exec_name);

        if !filename_abs.exists() {
            self.set_status(PmProcessStatus::InitializingFailed).await;
            return Err(anyhow::Error::msg(format!(
                "Executable not found: {}",
                filename_abs.display()
            )));
        }

        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let logs_dir = pm3_home_dir.join("processes").join(&cfg.proc_name);
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
            .current_dir(&cfg.exec_dir)
            .args(&cfg.exec_args)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;

        {
            let mut guard = self.inner.lock().await;
            guard.handle = Some(child);
        }

        self.set_status(PmProcessStatus::Running).await;

        Ok(())
    }

    pub async fn monitor(&self, sys: &mut System) {
        let (name, handle) = { (self.proc_name.clone(), &mut self.inner.lock().await.handle) };

        let prefix = format!("{name} [process]");

        let (pid_u32, exited) = {
            let Some(child) = handle.as_mut() else {
                eprintln!("[pm3] {prefix} no handle");
                return;
            };

            let pid = child.id().unwrap_or(0);

            let exited = match child.try_wait() {
                Ok(Some(status)) => {
                    println!("{prefix} exited: {status}");
                    *handle = None;
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

    pub async fn dump_config(&self) -> anyhow::Result<PathBuf> {
        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let config_file_path = pm3_home_dir
            .join("configs")
            .join(format!("{}.proc", self.proc_name));

        pm3_safe_cfg_handler::save_config(&self.inner.lock().await.config, &config_file_path)?;

        Ok(config_file_path)
    }

    pub async fn is_active(&self) -> bool {
        self.inner.lock().await.process_status.is_active()
    }

    pub async fn set_status(&self, status: PmProcessStatus) {
        self.inner.lock().await.process_status = status;
    }

    pub async fn not_initialized(&self) {
        self.set_status(PmProcessStatus::InitializingFailed).await;
    }
}
