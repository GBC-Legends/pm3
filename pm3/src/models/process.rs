#[derive(Debug, Clone)]
pub struct PmProcessStatusInfo {
    pub id: u64,
    pub name: String,
    pub status: ProcessStatus,

    pub pid: Option<u32>,
    pub uptime: Option<u64>,
    pub cpu: Option<f32>,
    pub mem: Option<u64>,
    pub user: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessStatus {
    Running,
    Stopped,
    Exited,
}