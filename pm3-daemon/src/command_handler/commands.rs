use tokio::sync::oneshot;

use crate::models::pm3_config::PmProcessConfig;

#[derive(Debug)]
pub enum RunnerCommand {
    Ping {
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    Start {
        config: PmProcessConfig,
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    Stop {
        stop_programs: Vec<String>,
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    List {
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
}
