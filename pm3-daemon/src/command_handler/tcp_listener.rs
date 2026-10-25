use crate::command_handler::commands::{CmdReply, RunnerCommand};
use crate::daemon_config::DaemonConfig;
use crate::utils::config_validator::verify_start_config;
use crate::utils::encryption::DecryptError;
use crate::utils::encryption::decrypt_wire_line;
use crate::utils::encryption::encrypt_reply_to_token;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

pub(crate) struct TcpCommandHandler {
    listener: TcpListener,
    tx: mpsc::Sender<RunnerCommand>,
    cfg: DaemonConfig,
}

static CFG: OnceLock<DaemonConfig> = OnceLock::new();

impl TcpCommandHandler {
    pub(crate) async fn new(
        config: &DaemonConfig,
        tx: mpsc::Sender<RunnerCommand>,
    ) -> anyhow::Result<Self> {
        let addr = format!("127.0.0.1:{}", config.port);
        let listener = TcpListener::bind(&addr).await?;
        println!("TcpCommandHandler is listening on {}", &addr);

        CFG.set(config.clone())
            .expect("TcpCommandHandler initialized twice");

        Ok(Self {
            listener,
            tx,
            cfg: config.clone(),
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let key = self.cfg.key();
        let aad: &'static [u8] = b"pm3:tcp:v1";

        loop {
            let (stream, addr) = self.listener.accept().await?;

            let tx = self.tx.clone();
            let key = key;
            let aad = aad;

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, tx, &key, aad).await {
                    eprintln!("Client {addr} error: {e:?}");
                }
            });
        }
    }

    async fn handle_client(
        stream: TcpStream,
        tx: mpsc::Sender<RunnerCommand>,
        key: &[u8; 32],
        aad: &[u8],
    ) -> anyhow::Result<()> {
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                break;
            }

            let cmd = line.trim();
            match Self::process_command(cmd, &tx).await {
                Ok(CmdReply::One(msg)) => {
                    let reply_plain = format!("OK {msg}\n");
                    let token = encrypt_reply_to_token(key, reply_plain.as_bytes(), aad);
                    write_half
                        .write_all(format!("ENC {token}\n").as_bytes())
                        .await?;
                }

                Ok(CmdReply::Stream(mut rx)) => {
                    let head = "OK LOGS\n";
                    let token = encrypt_reply_to_token(key, head.as_bytes(), aad);
                    write_half
                        .write_all(format!("ENC {token}\n").as_bytes())
                        .await?;

                    while let Some(item) = rx.recv().await {
                        let line = match item {
                            Ok(s) => format!("LOG {s}\n"),
                            Err(e) => format!("ERR {e}\n"),
                        };

                        let token = encrypt_reply_to_token(key, line.as_bytes(), aad);
                        if write_half
                            .write_all(format!("ENC {token}\n").as_bytes())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }

                    let eof = "OK EOF\n";
                    let token = encrypt_reply_to_token(key, eof.as_bytes(), aad);
                    let _ = write_half
                        .write_all(format!("ENC {token}\n").as_bytes())
                        .await;
                }

                Err(e) => {
                    let reply_plain = format!("ERR {e}\n");
                    let token = encrypt_reply_to_token(key, reply_plain.as_bytes(), aad);
                    write_half
                        .write_all(format!("ENC {token}\n").as_bytes())
                        .await?;
                }
            }
        }

        Ok(())
    }

    fn break_command(cmd: &str) -> (String, Vec<String>) {
        let key = CFG.get().expect("TcpCommandHandler not initialized").key();
        let aad: &'static [u8] = b"pm3:tcp:v1";

        let decrypted_cmd = match decrypt_wire_line(&key, &cmd, aad) {
            Ok(cmd) => cmd,
            Err(DecryptError::BadBase64(err)) => {
                eprintln!("decrypt error: bad base64: {err}");
                b"not_encrypted".to_vec()
            }
            Err(DecryptError::TooShort) => {
                eprintln!("decrypt error: payload too short");
                b"not_encrypted".to_vec()
            }
            Err(DecryptError::BadVersion(version)) => {
                eprintln!("decrypt error: bad version: {version}");
                b"not_encrypted".to_vec()
            }
            Err(DecryptError::Crypto) => {
                eprintln!("decrypt error: crypto failure");
                b"not_encrypted".to_vec()
            }
        };

        let cmd = String::from_utf8_lossy(&decrypted_cmd).into_owned();

        let mut parts = cmd.split_whitespace();

        let command = parts.next().unwrap_or("").to_lowercase();
        let args = parts.map(|s| s.to_string()).collect::<Vec<String>>();

        (command, args)
    }

    async fn process_command(
        cmd: &str,
        tx: &mpsc::Sender<RunnerCommand>,
    ) -> anyhow::Result<CmdReply> {
        let (command, args) = Self::break_command(cmd);

        match command.as_str() {
            "ping" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Ping { reply: reply_tx }).await?;
                Ok(CmdReply::One(reply_rx.await??))
            }

            "start" => {
                let cfg = match verify_start_config(args.first().unwrap()) {
                    Ok(cfg) => cfg,
                    Err(e) => return Err(e),
                };

                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Start {
                    reply: reply_tx,
                    config: cfg,
                })
                .await?;
                Ok(CmdReply::One(reply_rx.await??))
            }
            "stop" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Stop {
                    stop_programs: args
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                    reply: reply_tx,
                })
                .await?;
                Ok(CmdReply::One(reply_rx.await??))
            }
            "list" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::List { reply: reply_tx }).await?;
                Ok(CmdReply::One(reply_rx.await??))
            }
            "list-programs" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::ListPrograms { reply: reply_tx })
                    .await?;
                Ok(CmdReply::One(reply_rx.await??))
            }

            "logs" => {
                let mut lines = 25;
                let mut programs = Vec::new();

                for arg in args {
                    if let Some(v) = arg.strip_prefix("--lines=") {
                        if let Ok(n) = v.parse::<u64>() {
                            lines = n;
                        }
                    } else {
                        programs.push(arg);
                    }
                }

                let (reply_tx, reply_rx) = mpsc::unbounded_channel();

                tx.send(RunnerCommand::Logs {
                    stream: reply_tx,
                    lines: lines,
                    programs: programs
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                })
                .await?;
                Ok(CmdReply::Stream(reply_rx))
            }

            "restart" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Restart {
                    reply: reply_tx,
                    programs: args
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                })
                .await?;
                Ok(CmdReply::One(reply_rx.await??))
            }

            "delete" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Delete {
                    reply: reply_tx,
                    programs: args
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                })
                .await?;
                Ok(CmdReply::One(reply_rx.await??))
            }

            "revive" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Revive { reply: reply_tx }).await?;
                Ok(CmdReply::One(reply_rx.await??))
            }

            "dump" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Dump { reply: reply_tx }).await?;
                Ok(CmdReply::One(reply_rx.await??))
            }

            "flush" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Flush {
                    reply: reply_tx,
                    programs: args
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                })
                .await?;
                Ok(CmdReply::One(reply_rx.await??))
            }
            _ => anyhow::bail!("unknown command"),
        }
    }
}
