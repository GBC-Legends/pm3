use crate::command_handler::commands::RunnerCommand;
use crate::logging::LogChunk;
use crate::logging::logging_subscription::LoggingSubscriptionAction;
use crate::metrics::metrics_service::MetricsService;
use crate::process_runner::idx;
use crate::process_runner::pm3_process::PmProcess;
use crate::utils::pm3_safe_dir;
use anyhow::Result;
use rand::{Rng, distributions::Alphanumeric};
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::time::{Duration, interval};

pub struct ProcessRunner {
    pub subs_sender: mpsc::Sender<LoggingSubscriptionAction>,
    pub processes: Vec<Arc<PmProcess>>,
}

impl ProcessRunner {
    pub fn init(
        subs_sender: mpsc::Sender<LoggingSubscriptionAction>,
    ) -> (Self, Vec<(u64, String)>) {
        let slf = ProcessRunner {
            subs_sender,
            processes: Vec::new(),
        };

        let processes_metrics = Vec::new();

        return (slf, processes_metrics);
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

                    if let Err(e) = self.handle_command(cmd, &mut sys).await {
                        eprintln!("[pm3] handle_command error: {e:?}");
                    }
                }
            }
        }
    }

    async fn handle_command(
        &mut self,
        cmd: RunnerCommand,
        system: &mut System,
    ) -> anyhow::Result<()> {
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

                if let Err(e) = process.awake().await {
                    let _ = reply.send(Err(e.into()));
                    return Ok(());
                }

                if let Err(e) =
                    MetricsService::sync_new_process((process.idx, process.proc_name.to_string()))
                        .await
                {
                    let _ = reply.send(Err(anyhow::anyhow!(e)));
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
                let mut lines: Vec<String> = Vec::with_capacity(self.processes.len());

                for process in &self.processes {
                    match process.get_current_status(system).await {
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
                    out.push(format!("{} {}", p_handle.idx, p_handle.proc_name));
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
            RunnerCommand::Restart {
                programs: restart_programs,
                reply,
            } => {
                let mut finished = Vec::<String>::new();

                for program_id in restart_programs {
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
                        match proc.restart().await {
                            Ok(()) => finished.push(proc.idx.to_string()),
                            Err(e) => println!("Error restarting process: {e}"),
                        }
                    }
                }

                let msg = if finished.is_empty() {
                    "No processes were restarted".to_string()
                } else {
                    format!("Restarted {} successfully", finished.join(", "))
                };

                let _ = reply.send(Ok(msg));
                Ok(())
            }
            RunnerCommand::Delete {
                programs: delete_programs,
                reply,
            } => {
                let mut finished = Vec::<String>::new();

                for program_id in delete_programs {
                    let Some(id) = program_id.parse::<u64>().ok() else {
                        continue;
                    };

                    let Some(pos) = self.processes.iter().position(|p| p.idx == id) else {
                        continue;
                    };

                    {
                        let proc = &self.processes[pos];

                        if !proc.is_active().await {
                            println!("process {program_id} is already stopped");
                        } else {
                            match proc.stop().await {
                                Ok(()) => {}
                                Err(e) => println!("Error stopping process: {e}"),
                            }
                        }
                    }

                    let removed = self.processes.remove(pos);
                    finished.push(removed.idx.to_string());
                }

                let msg = if finished.is_empty() {
                    "No processes were deleted".to_string()
                } else {
                    format!("Deleted {} successfully", finished.join(", "))
                };

                let _ = reply.send(Ok(msg));
                Ok(())
            }
            RunnerCommand::Flush { programs, reply } => {
                let (oneshot_tx, oneshot_rx) = oneshot::channel();
                let shared_sender = self.subs_sender.clone();

                let sub_id: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(5)
                    .map(char::from)
                    .collect();

                let sub = LoggingSubscriptionAction::Truncate {
                    id: sub_id,
                    programs,
                    oneshot_tx,
                };

                if shared_sender.send(sub).await.is_err() {
                    let _ = reply.send(Err(anyhow::anyhow!("channel closed")));
                    return Ok(());
                }

                let msg = oneshot_rx.await?;
                let _ = reply.send(msg);

                Ok(())
            }
            RunnerCommand::Dump { reply } => {
                let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();
                let configs_dir = pm3_home_dir.join("configs");
                let configs_old_dir = pm3_home_dir.join("configs.old");
                let mut cnt = 0;

                let result: anyhow::Result<String> = async {
                    if tokio::fs::try_exists(&configs_old_dir).await? {
                        tokio::fs::remove_dir_all(&configs_old_dir).await?;
                    }

                    if tokio::fs::try_exists(&configs_dir).await? {
                        tokio::fs::rename(&configs_dir, &configs_old_dir).await?;
                    }

                    tokio::fs::create_dir_all(&configs_dir).await?;
                    for proc in &self.processes {
                        proc.dump_config().await?;
                        cnt += 1;
                    }

                    Ok(format!(
                        "{} configs saved to ~/.pm3/configs (backup created in ~/.pm3/configs.old)",
                        cnt
                    ))
                }
                .await;

                let _ = reply.send(result);
                Ok(())
            }
            RunnerCommand::Revive { reply } => {
                if !self.processes.is_empty() {
                    for proc in self.processes.iter() {
                        if proc.is_active().await {
                            proc.stop().await?;
                        }
                    }

                    self.processes.clear();
                    // Reset the NEXT_ID counter to 1
                    use crate::process_runner::idx::NEXT_ID;
                    use std::sync::atomic::Ordering;
                    NEXT_ID.store(1, Ordering::Relaxed);
                }

                let mut cnt = 0;
                use crate::utils::pm3_safe_cfg_handler;

                let configs_dir = pm3_safe_cfg_handler::parse_configs().unwrap();

                let mut metrics_processes = Vec::with_capacity(configs_dir.len());

                for cfg in configs_dir {
                    let process = PmProcess::new(cfg, idx::alloc_id());
                    metrics_processes.push((process.idx, process.proc_name.to_string()));
                    self.processes.push(Arc::new(process));
                    cnt += 1;
                }

                MetricsService::sync_processes(metrics_processes).await?;
                self.run().await?;

                let msg = format!("PM3 has started {} processes from ~/.pm3/configs", cnt);
                let _ = reply.send(Ok(msg));
                Ok(())
            }
        }
    }
}
