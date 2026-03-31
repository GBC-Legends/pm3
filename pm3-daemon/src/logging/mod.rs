pub mod logging_instance;
pub mod logging_service;
pub mod logging_subscription;

use std::fmt::Display;
use std::path::PathBuf;

#[allow(unused)]
pub struct ProcLogMeta {
    pub proc_name: String,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

#[derive(Clone, Copy, Debug)]
pub enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone)]
pub enum LogChunk {
    Line(String),
    Eof,
    Ping,
}

impl Display for LogChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogChunk::Line(s) => write!(f, "{s}"),
            LogChunk::Eof => write!(f, "EOF"),
            LogChunk::Ping => write!(f, "PING"),
        }
    }
}
