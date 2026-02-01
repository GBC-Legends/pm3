use crate::process_runner::pm3_process::PmProcess;

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

    pub async fn run(self: &Self) {
        for process in &self.processes {
            if process.config.active {
                process.awake();
            }
        }
    }
}
