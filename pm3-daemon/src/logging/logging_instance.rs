use std::io::Read;

use crate::logging::StreamKind;

use os_pipe::{PipeReader, PipeWriter, pipe};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct LogMsg {
    pub idx: u64,
    pub stream: StreamKind,
    pub bytes: Vec<u8>,
}

pub struct LoggingInstance {
    writer: Option<PipeWriter>,
    _task: tokio::task::JoinHandle<()>,
}

impl LoggingInstance {
    pub fn new(
        proc_name: String,
        idx: u64,
        stream: StreamKind,
        tx: mpsc::UnboundedSender<LogMsg>,
        handle: Handle,
    ) -> Self {
        let (reader, writer) = pipe().expect("pipe failed");
        let task = Self::spawn_reader(reader, proc_name, idx, stream, tx, handle);

        Self {
            writer: Some(writer),
            _task: task,
        }
    }

    fn spawn_reader(
        reader: PipeReader,
        proc_name: String,
        idx: u64,
        stream: StreamKind,
        tx: mpsc::UnboundedSender<LogMsg>,
        handle: Handle,
    ) -> tokio::task::JoinHandle<()> {
        handle.spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx
                            .send(LogMsg {
                                idx: idx,
                                stream,
                                bytes: buf[..n].to_vec(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    }
}

impl From<LoggingInstance> for std::process::Stdio {
    fn from(mut instance: LoggingInstance) -> std::process::Stdio {
        let writer = instance.writer.take().expect("already taken");
        std::process::Stdio::from(writer)
    }
}
