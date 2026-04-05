use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::logging::LogChunk;
use crate::models::pm3_config::PmProcessConfig;

pub enum CmdReply {
    One(String),
    Stream(mpsc::UnboundedReceiver<anyhow::Result<LogChunk>>),
}

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
    ListPrograms {
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    Logs {
        lines: u64,
        programs: Vec<String>,
        stream: mpsc::UnboundedSender<anyhow::Result<LogChunk>>,
    },
    Restart {
        programs: Vec<String>,
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    Delete {
        programs: Vec<String>,
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    Flush {
        programs: Vec<String>,
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
    Dump {
        reply: oneshot::Sender<anyhow::Result<String>>,
    },
}
