use crate::command_handler::commands::RunnerCommand;
use crate::process_runner::pm3_process::PmProcess;
use crate::utils::bytes_safe_formatting::format_bytes;
use anyhow::Result;
use std::sync::Arc;
use sysinfo::{Pid, System};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{Duration, interval};

pub struct ProcessRunner {
    pub processes: Vec<Arc<Mutex<PmProcess>>>,
}

impl ProcessRunner {
    pub fn init() -> Self {
        let mut slf = ProcessRunner {
            processes: Vec::new(),
        };
        use crate::utils::pm3_safe_cfg_handler;

        let configs_dir = pm3_safe_cfg_handler::parse_configs().unwrap();

        for cfg in configs_dir {
            let process = PmProcess::new(cfg);
            slf.processes.push(Arc::new(Mutex::new(process)));
        }

        return slf;
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut set = JoinSet::new();

        for process in &self.processes {
            let process = Arc::clone(process);

            set.spawn(async move {
                let active = {
                    let p = process.lock().await;
                    p.config.active
                };

                if !active {
                    return;
                }

                let mut p = process.lock().await;
                if let Err(e) = p.awake().await {
                    eprintln!("[pm3] awake failed: {e}");
                }
            });
        }

        while let Some(res) = set.join_next().await {
            if let Err(e) = res {
                eprintln!("[pm3] task join error: {e}");
            }
        }

        Ok(())
    }

    pub async fn dispatch(&mut self, rx: &mut mpsc::Receiver<RunnerCommand>) {
        println!("Dispatching processes...");
        let mut sys = System::new();
        let mut tick = interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                biased;
                _ = tick.tick() => {
                    for p in &self.processes {
                        let (name, handle) = {
                            let proc = p.lock().await;
                            (proc.config.exec_name.clone(), Arc::clone(&proc.handle))
                        };

                        let prefix = format!("{name} [process]");

                        let (pid_u32, exited) = {
                            let mut h = handle.lock().await;

                            let Some(child) = h.as_mut() else {
                                eprintln!("[pm3] {prefix} no handle");
                                continue;
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
                            continue;
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
                }

                cmd = rx.recv() => {
                    let Some(cmd) = cmd else {
                        eprintln!("[pm3] command channel closed");
                        break;
                    };

                    if let Err(e) = self.handle_command(cmd).await {
                        eprintln!("[pm3] handle_command error: {e:?}");
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, cmd: RunnerCommand) -> anyhow::Result<()> {
        match cmd {
            RunnerCommand::Ping { reply } => {
                let _ = reply.send(Ok("pong".to_string()));
                Ok(())
            }
        }
    }
}
