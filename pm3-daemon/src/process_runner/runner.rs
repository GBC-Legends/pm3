use crate::command_handler::commands::RunnerCommand;
use crate::logging::LogChunk;
use crate::logging::logging_subscription::LoggingSubscriptionAction;
use crate::process_runner::idx;
use crate::process_runner::pm3_process::PmProcess;
use anyhow::Result;
use rand::{Rng, distributions::Alphanumeric};
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{Duration, interval};

pub struct ProcessRunner {
    pub subs_sender: mpsc::Sender<LoggingSubscriptionAction>,
    pub processes: Vec<Arc<PmProcess>>,
}

impl ProcessRunner {
    pub fn init(subs_sender: mpsc::Sender<LoggingSubscriptionAction>) -> Self {
        let mut slf = ProcessRunner {
            subs_sender,
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
            RunnerCommand::List { reply } => {
                let mut system = System::new();

                let mut lines: Vec<String> = Vec::with_capacity(self.processes.len());

                for process in &self.processes {
                    match process.get_current_status(&mut system).await {
                        Ok(info) => lines.push(info.to_qs_line()),
                        Err(e) => {
                            lines.push(format!("status=error&msg={}", e));
                        }
                    }
                }

                let mut out = String::new();
                out.push_str(&lines.len().to_string());
                out.push('\n');

                if !lines.is_empty() {
                    out.push_str(&lines.join("\n"));
                    out.push('\n');
                }

                let _ = reply.send(Ok(out));
                Ok(())
            }
            RunnerCommand::ListPrograms { reply } => {
                let mut out = Vec::new();
                for process in &self.processes {
                    let p_handle = process.clone();
                    out.push(p_handle.idx.to_string());
                }
                let _ = reply.send(Ok(out.join(" ")));
                Ok(())
            }
            RunnerCommand::Logs {
                stream,
                lines,
                programs,
            } => {
                let shared_sender = self.subs_sender.clone();

                tokio::spawn(async move {
                    let (tx, mut rx) = mpsc::unbounded_channel();

                    let sub_id: String = rand::thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(5)
                        .map(char::from)
                        .collect();

                    let subscription = LoggingSubscriptionAction::Subscribe {
                        id: sub_id.clone(),
                        tx,
                        programs,
                        lines,
                    };

                    if shared_sender.send(subscription).await.is_err() {
                        let _ = stream.send(Ok(LogChunk::Eof));
                        return;
                    }

                    let mut ping = interval(Duration::from_secs(15));

                    ping.tick().await;

                    loop {
                        tokio::select! {
                            _ = ping.tick() => {
                                if stream.send(Ok(LogChunk::Ping)).is_err() {
                                    println!("client disconnected by ping, unsubscribing: {sub_id}");
                                    let _ = shared_sender
                                        .send(LoggingSubscriptionAction::Unsubscribe {
                                            id: sub_id.clone(),
                                        })
                                        .await;
                                    return;
                                }
                            }

                            maybe_chunk = rx.recv() => {
                                match maybe_chunk {
                                    Some(LogChunk::Line(line)) => {
                                        if stream.send(Ok(LogChunk::Line(line))).is_err() {
                                            println!("client disconnected, unsubscribing: {sub_id}");
                                            let _ = shared_sender
                                                .send(LoggingSubscriptionAction::Unsubscribe {
                                                    id: sub_id.clone(),
                                                })
                                                .await;
                                            return;
                                        }
                                    }
                                    Some(LogChunk::Eof) => {
                                        let _ = stream.send(Ok(LogChunk::Eof));
                                        let _ = shared_sender
                                            .send(LoggingSubscriptionAction::Unsubscribe {
                                                id: sub_id.clone(),
                                            })
                                            .await;
                                        return;
                                    },
                                    Some(LogChunk::Ping) => {},
                                    None => {
                                        println!("subscription source closed, unsubscribing: {sub_id}");
                                        let _ = shared_sender
                                            .send(LoggingSubscriptionAction::Unsubscribe {
                                                id: sub_id.clone(),
                                            })
                                            .await;

                                        let _ = stream.send(Ok(LogChunk::Eof));
                                        return;
                                    }
                                }
                            }
                        }
                    }
                });

                Ok(())
            }
        }
    }
}
