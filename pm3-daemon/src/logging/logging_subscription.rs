use tokio::sync::mpsc;

pub struct LoggingSubscription {
    pub id: String,
    pub tx: mpsc::UnboundedSender<String>,
    pub programs: Vec<String>,
    pub lines: u64,
}

pub enum LoggingSubscriptionAction {
    Subscribe {
        id: String,
        tx: mpsc::UnboundedSender<String>,
        programs: Vec<String>,
        lines: u64,
    },
    Unsubscribe {
        id: String,
    },
}
