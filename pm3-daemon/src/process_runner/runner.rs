use crate::command_handler::commands::RunnerCommand;
use crate::process_runner::idx;
use crate::process_runner::pm3_process::PmProcess;
use anyhow::Result;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{Duration, interval};

pub struct ProcessRunner {
    pub processes: Vec<Arc<PmProcess>>,
}

impl ProcessRunner {
    pub fn init() -> Self {
        let mut slf = ProcessRunner {
            processes: Vec::new(),
        };
        use crate::utils::pm3_safe_cfg_handler;

        let configs_dir = pm3_safe_cfg_handler::parse_configs().unwrap();

        for cfg in configs_dir {
            let process = PmProcess::new(cfg, idx::alloc_id());
            slf.processes.push(Arc::new(process));
        }

        return slf;
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut set = JoinSet::new();

        for process in &self.processes {
            let process = Arc::clone(process);

            set.spawn(async move {
                let active = process.is_active().await;
                if !active {
                    return;
                }

                if let Err(e) = process.awake().await {
                    process.not_initialized().await;
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
                        if p.is_active().await {
                            p.monitor(&mut sys).await;
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
            RunnerCommand::Start { config, reply } => {
                let mut exists = false;
                for p in &self.processes {
                    let process = Arc::clone(p);
                    if process.proc_name.as_ref() == config.proc_name {
                        exists = true;
                        break;
                    }
                }
                if exists {
                    let _ = reply.send(Ok("Process already exists".to_string()));
                    return Ok(());
                }

                let process = PmProcess::new(config.clone(), idx::alloc_id());

                if let Err(e) = process.dump_config().await {
                    let _ = reply.send(Err(e.into()));
                    return Ok(());
                }

                if let Err(e) = process.awake().await {
                    let _ = reply.send(Err(e.into()));
                    return Ok(());
                }

                let process_arc = Arc::new(process);

                self.processes.push(process_arc);

                let _ = reply.send(Ok("Started successfully".to_string()));

                Ok(())
            }
            RunnerCommand::Stop {
                stop_programs,
                reply,
            } => {
                let mut finished = Vec::<String>::new();

                for program_id in stop_programs {
                    let proc_opt = program_id
                        .parse::<u64>()
                        .ok()
                        .and_then(|id| self.processes.iter().find(|p| p.idx == id))
                        .or_else(|| {
                            self.processes
                                .iter()
                                .find(|p| p.proc_name.as_ref() == program_id)
                        });

                    if let Some(proc) = proc_opt {
                        if !proc.is_active().await {
                            println!("process {program_id} is already stopped");
                            continue;
                        }

                        match proc.stop().await {
                            Ok(()) => finished.push(proc.idx.to_string()),
                            Err(e) => println!("Error stopping process: {e}"),
                        }
                    }
                }

                let msg = if finished.is_empty() {
                    "No processes were stopped".to_string()
                } else {
                    format!("Stopped {} successfully", finished.join(", "))
                };

                let _ = reply.send(Ok(msg));
                Ok(())
            }
        }
    }
}
