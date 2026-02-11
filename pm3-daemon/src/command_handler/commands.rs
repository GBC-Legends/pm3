use tokio::sync::oneshot;

#[derive(Debug)]
pub enum RunnerCommand {
    Ping {
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
}
