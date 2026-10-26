use std::io::Read;

use crate::logging::StreamKind;

use os_pipe::{PipeReader, PipeWriter, pipe};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

#[derive(Debug)]
pub struct LogMsg {
    pub idx: u64,
    pub stream: StreamKind,
    pub bytes: Vec<u8>,
}

pub struct LoggingInstance {
    writer: Option<PipeWriter>,
}

impl LoggingInstance {
    pub fn new(
        idx: u64,
        stream: StreamKind,
        tx: mpsc::UnboundedSender<LogMsg>,
        readers: &mut JoinSet<std::io::Result<()>>,
    ) -> Self {
        let (reader, writer) = pipe().expect("pipe failed");
        Self::spawn_reader(reader, idx, stream, tx, readers);

        Self {
            writer: Some(writer),
        }
    }

    fn spawn_reader(
        reader: PipeReader,
        idx: u64,
        stream: StreamKind,
        tx: mpsc::UnboundedSender<LogMsg>,
        readers: &mut JoinSet<std::io::Result<()>>,
    ) {
        readers.spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => return Ok(()),
                    Ok(n) => {
                        if tx
                            .send(LogMsg {
                                idx: idx,
                                stream,
                                bytes: buf[..n].to_vec(),
                            })
                            .is_err()
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::BrokenPipe,
                                "log receiver closed before pipe was drained",
                            ));
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        });
    }
}

impl From<LoggingInstance> for std::process::Stdio {
    fn from(mut instance: LoggingInstance) -> std::process::Stdio {
        let writer = instance.writer.take().expect("already taken");
        std::process::Stdio::from(writer)
    }
}
