use crate::logging::logging_service::LoggingService;
use crate::utils::bytes_safe_formatting::format_bytes;
use crate::utils::get_process_users::username_for_pid;
use crate::utils::pm3_safe_dir;
use crate::{models::pm3_config::PmProcessConfig, utils::pm3_safe_cfg_handler};
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Pid, System};
use tokio::sync::Mutex;

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

#[derive(Debug, Default, Clone)]
pub enum PmProcessStatus {
    Disabled,
    #[default]
    NotStarted,
    Initializing,
    InitializingFailed,
    Running,
    Stopped,
    Exited(i32),
    Finished(i32),
}

#[derive(Debug, Default)]
pub struct PmProcessStatusInfo {
    id: u64,
    name: String,
    status: PmProcessStatus,
    pid: Option<u32>,
    uptime: Option<u64>,
    cpu_usage: Option<f32>,
    memory_usage: Option<u64>,
    user: Option<String>,
}

impl PmProcessStatusInfo {
    pub fn to_qs_line(&self) -> String {
        fn enc(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + s.len() / 2);
            for &b in s.as_bytes() {
                let ok = matches!(
                    b,
                    b'A'..=b'Z'
                        | b'a'..=b'z'
                        | b'0'..=b'9'
                        | b'-'
                        | b'.'
                        | b'_'
                        | b'~'
                );
                if ok {
                    out.push(b as char);
                } else {
                    out.push('%');
                    out.push_str(&format!("{:02X}", b));
                }
            }
            out
        }

        fn push_kv(dst: &mut String, k: &str, v: &str) {
            if !dst.is_empty() {
                dst.push('&');
            }
            dst.push_str(&enc(k));
            dst.push('=');
            dst.push_str(&enc(v));
        }

        fn push_opt<T: ToString>(dst: &mut String, k: &str, v: Option<T>) {
            if let Some(v) = v {
                push_kv(dst, k, &v.to_string());
            }
        }

        let mut s = String::new();

        push_kv(&mut s, "id", &self.id.to_string());
        push_kv(&mut s, "name", &self.name);

        let mut exit_code: Option<i32> = None;
        let status = match self.status {
            PmProcessStatus::Running => "running",
            PmProcessStatus::Exited(code) | PmProcessStatus::Finished(code) => {
                exit_code = Some(code);
                "exited"
            }
            _ => "stopped",
        };
        push_kv(&mut s, "status", status);
        push_opt(&mut s, "exit_code", exit_code);

        push_opt(&mut s, "pid", self.pid);
        push_opt(&mut s, "uptime", self.uptime);

        if let Some(cpu) = self.cpu_usage {
            push_kv(&mut s, "cpu", &format!("{:.5}", cpu));
        }

        push_opt(&mut s, "mem", self.memory_usage);

        if let Some(user) = &self.user {
            push_kv(&mut s, "user", user);
        }

        s
    }
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

        let (stdout, stderr) = LoggingService::get_logging_pair(&cfg.proc_name);

        let child = Command::new(&filename_abs)
            .current_dir(&cfg.exec_dir)
            .args(&cfg.exec_args)
            .env("PYTHONUNBUFFERED", "1")
            .stdout(stdout)
            .stderr(stderr)
            .spawn()?;

        {
            let mut guard = self.inner.lock().await;
            guard.handle = Some(child);
        }

        self.set_status(PmProcessStatus::Running).await;

        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut child = {
            let mut guard = self.inner.lock().await;
            guard.handle.take()
        };

        let Some(child_ref) = child.as_mut() else {
            return Err(anyhow::Error::msg(format!(
                "Error stopping program: {}({})",
                self.proc_name.as_ref(),
                self.idx
            )));
        };

        if let Err(e) = child_ref.kill().await {
            let mut guard = self.inner.lock().await;
            guard.handle = child;
            return Err(anyhow::Error::new(e));
        }

        self.set_status(PmProcessStatus::Stopped).await;
        Ok(())
    }

    pub async fn get_current_status(
        &self,
        sys: &mut System,
    ) -> anyhow::Result<PmProcessStatusInfo> {
        let process_guard = self.inner.lock().await;

        let Some(child) = process_guard.handle.as_ref() else {
            return Ok(PmProcessStatusInfo {
                id: self.idx,
                name: self.proc_name.to_string(),
                status: PmProcessStatus::Stopped,
                pid: None,
                uptime: None,
                cpu_usage: None,
                memory_usage: None,
                user: None,
            });
        };

        let Some(pid_u32) = child.id() else {
            return Ok(PmProcessStatusInfo {
                id: self.idx,
                name: self.proc_name.to_string(),
                ..Default::default()
            });
        };

        let pid = Pid::from_u32(pid_u32);

        sys.refresh_process(pid);

        let Some(p) = sys.process(pid) else {
            return Ok(PmProcessStatusInfo {
                id: self.idx,
                name: self.proc_name.to_string(),
                status: process_guard.process_status.clone(),
                pid: Some(pid_u32),
                uptime: None,
                cpu_usage: None,
                memory_usage: None,
                user: None,
            });
        };

        let uptime = p.run_time();

        let cpu = p.cpu_usage();

        let mem = p.memory();

        let user = username_for_pid(&pid);

        Ok(PmProcessStatusInfo {
            id: self.idx,
            name: self.proc_name.to_string(),
            status: process_guard.process_status.clone(),
            pid: Some(pid_u32),
            uptime: Some(uptime),
            cpu_usage: Some(cpu),
            memory_usage: Some(mem),
            user,
        })
    }

    pub async fn monitor(&self, sys: &mut System) {
        let name = self.proc_name.clone();
        let prefix = format!("{name} [process]");

        let mut guard = self.inner.lock().await;

        let Some(child) = guard.handle.as_mut() else {
            if guard.process_status.is_active() {
                eprintln!("[pm3] {prefix} no handle");
            }
            return;
        };

        let pid_u32 = child.id().unwrap_or(0);

        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);

                guard.handle = None;

                if code == 0 {
                    guard.process_status = PmProcessStatus::Finished(code);
                } else {
                    guard.process_status = PmProcessStatus::Exited(code);
                }

                let printed_code = match guard.process_status {
                    PmProcessStatus::Finished(c) | PmProcessStatus::Exited(c) => c,
                    _ => -1,
                };

                println!("Process {name} exited with code {printed_code}");
                return;
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("[pm3] {prefix} try_wait error: {e}");
                return;
            }
        }

        drop(guard);

        let pid = Pid::from_u32(pid_u32);
        sys.refresh_process(pid);

        if let Some(proc_) = sys.process(pid) {
            let mem_mb = proc_.memory() as f64;
            let cpu = proc_.cpu_usage();
            use crate::metrics::metrics_service::MetricsLog;
            use crate::metrics::metrics_service::MetricsService;

            let metrics_log = MetricsLog {
                proc_name: self.proc_name.as_ref().to_string(),
                cpu_usage: cpu,
                memory_usage: mem_mb as u64,
            };

            match MetricsService::get_metrics_handle().send(metrics_log).await {
                Ok(_) => (),
                Err(e) => eprintln!("Failed to send metrics log: {e}"),
            };

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
