use crate::process_runner::pm3_process::PmProcess;
use tokio::task::JoinSet;

pub struct ProcessRunner {
    pub processes: Vec<crate::process_runner::pm3_process::PmProcess>,
}

impl ProcessRunner {
    pub fn init() -> Self {
        let mut slf = ProcessRunner {
            processes: Vec::new(),
        };
        use crate::utils::pm3_safe_cfg_handler;

        let configs_dir = pm3_safe_cfg_handler::parse_configs().unwrap();

        // println!("{configs_dir:?}");
        for cfg in configs_dir {
            let process = PmProcess::new(cfg);
            slf.processes.push(process);
        }

        return slf;
    }

    pub async fn run(&self) {
        let mut set = JoinSet::new();

        for process in &self.processes {
            if !process.config.active {
                continue;
            }

            let cfg = process.config.clone();

            set.spawn(async move {
                let p = PmProcess::new(cfg);
                let _ = p.awake().await;
            });
        }

        while let Some(res) = set.join_next().await {
            if let Err(e) = res {
                eprintln!("[pm3] task join error: {e}");
            }
        }
    }
}
