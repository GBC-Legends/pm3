use crate::models::pm3_config::PmProcessConfig;
use crate::utils::pm3_safe_dir;

use std::{fs::File, process::Stdio};
use sysinfo::{Pid, System};
use tokio::{
    process::Command,
    time::{Duration, sleep},
};

pub struct PmProcess {
    pub config: PmProcessConfig,
}

impl PmProcess {
    pub fn new(cfg: PmProcessConfig) -> Self {
        PmProcess { config: cfg }
    }

    pub async fn awake(&self) -> std::io::Result<()> {
        let proc_name = self.config.exec_name.as_str();
        let prefix = format!("[pm3:{proc_name}]");

        let filename_abs = self
            .config
            .exec_dir_absolute_path
            .join(&self.config.exec_name);

        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let logs_dir = pm3_home_dir.join("processes").join(proc_name);
        tokio::fs::create_dir_all(&logs_dir).await?;

        let stdout_path = logs_dir.join("stdout.log");
        let stderr_path = logs_dir.join("stderr.log");

        // Можно оставить std::fs::File: создаётся один раз, не критично.
        let stdout = File::create(&stdout_path)?;
        let stderr = File::create(&stderr_path)?;

        let mut child = Command::new(&filename_abs)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;

        let pid = Pid::from_u32(child.id().unwrap_or(0));
        let mut sys = System::new();

        println!(
            "{prefix} started {:?} pid={}",
            filename_abs,
            child.id().unwrap_or(0)
        );

        loop {
            // tokio-friendly ожидание
            match child.try_wait()? {
                Some(status) => {
                    println!("{prefix} exited: {status}");
                    break;
                }
                None => {}
            }

            // sysinfo синхронный, но лёгкий; обычно ок в таком цикле
            sys.refresh_process(pid);

            if let Some(proc_) = sys.process(pid) {
                let mem_mb = proc_.memory() as f64 / 1024.0; // sysinfo memory обычно в KiB
                let cpu = proc_.cpu_usage();

                println!(
                    "{prefix} [monitor] CPU: {:.2}% | RAM: {:.2} MB",
                    cpu, mem_mb
                );
            } else {
                println!("{prefix} process not found (pid={})", pid.as_u32());
                break;
            }

            sleep(Duration::from_secs(1)).await;
        }

        Ok(())
    }
}
