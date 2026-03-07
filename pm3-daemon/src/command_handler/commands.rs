use std::fmt::Display;

use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::models::pm3_config::PmProcessConfig;

#[derive(Debug, Clone)]
pub enum LogChunk {
    Line(String),
    Eof,
}

impl Display for LogChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogChunk::Line(s) => write!(f, "{s}"),
            LogChunk::Eof => write!(f, "EOF"),
        }
    }
}

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
}
