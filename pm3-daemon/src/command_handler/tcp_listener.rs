use crate::command_handler::commands::RunnerCommand;
use crate::daemon_config::DaemonConfig;
use crate::utils::config_validator::verify_start_config;
use crate::utils::encryption::encrypt_reply_to_token;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

pub(crate) struct TcpCommandHandler {
    listener: TcpListener,
    tx: mpsc::Sender<RunnerCommand>,
    cfg: DaemonConfig,
}

impl TcpCommandHandler {
    pub(crate) async fn new(
        config: &DaemonConfig,
        tx: mpsc::Sender<RunnerCommand>,
    ) -> anyhow::Result<Self> {
        let addr = format!("127.0.0.1:{}", config.port);
        let listener = TcpListener::bind(&addr).await?;
        println!("Listening on {}", &addr);
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
            println!("New client: {addr}");

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
            let reply_plain = match Self::process_command(cmd, &tx).await {
                Ok(msg) => format!("OK {msg}\n"),
                Err(e) => format!("ERR {e}\n"),
            };

            let token = encrypt_reply_to_token(key, reply_plain.as_bytes(), aad);
            let out_line = format!("ENC {token}\n");
            write_half.write_all(out_line.as_bytes()).await?;
        }

        Ok(())
    }

    fn break_command(cmd: &str) -> (String, Vec<&str>) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts[0].to_lowercase();
        let args = parts[1..].to_vec();
        (command, args)
    }

    async fn process_command(
        cmd: &str,
        tx: &mpsc::Sender<RunnerCommand>,
    ) -> anyhow::Result<String> {
        let (command, args) = Self::break_command(cmd);

        match command.as_str() {
            "ping" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::Ping { reply: reply_tx }).await?;
                Ok(reply_rx.await??)
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
                Ok(reply_rx.await??)
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
                Ok(reply_rx.await??)
            }
            "list" => {
                let (reply_tx, reply_rx) = oneshot::channel();
                tx.send(RunnerCommand::List { reply: reply_tx }).await?;
                Ok(reply_rx.await??)
            }

            _ => anyhow::bail!("unknown command"),
        }
    }
}
