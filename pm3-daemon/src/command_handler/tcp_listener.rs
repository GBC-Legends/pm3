use crate::command_handler::commands::RunnerCommand;
use crate::utils::config_validator::verify_start_config;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

pub(crate) struct TcpCommandHandler {
    listener: TcpListener,
    tx: mpsc::Sender<RunnerCommand>,
}

impl TcpCommandHandler {
    pub(crate) async fn new(
        address: &str,
        tx: mpsc::Sender<RunnerCommand>,
    ) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(address).await?;
        println!("Listening on {}", address);
        Ok(Self { listener, tx })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            println!("New client: {addr}");

            let tx = self.tx.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, tx).await {
                    eprintln!("Client {addr} error: {e:?}");
                }
            });
        }
    }

    async fn handle_client(
        stream: TcpStream,
        tx: mpsc::Sender<RunnerCommand>,
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
            let reply = match Self::process_command(cmd, &tx).await {
                Ok(msg) => format!("OK {msg}\n"),
                Err(e) => format!("ERR {e}\n"),
            };

            write_half.write_all(reply.as_bytes()).await?;
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
                println!("{args:?}");
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
