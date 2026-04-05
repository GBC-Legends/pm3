use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::logging::LogChunk;
use std::collections::HashSet;

pub struct LoggingSubscription {
    pub id: String,
    pub tx: mpsc::UnboundedSender<LogChunk>,
    pub programs: HashSet<u64>,
}

pub enum LoggingSubscriptionAction {
    Subscribe {
        id: String,
        tx: mpsc::UnboundedSender<LogChunk>,
        programs: Vec<String>,
        lines: u64,
    },
    Unsubscribe {
        id: String,
    },
    Truncate {
        id: String,
        programs: Vec<String>,
        oneshot_tx: oneshot::Sender<anyhow::Result<String>>,
    },
}
