use tokio::sync::mpsc;

use crate::logging::LogChunk;

pub struct LoggingSubscription {
    pub id: String,
    pub tx: mpsc::UnboundedSender<LogChunk>,
    pub programs: Vec<String>,
    pub lines: u64,
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
}
