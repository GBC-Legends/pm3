use crate::models::pm3_config::PmProcessConfig;
use crate::utils::pm3_safe_dir;
use std::{
    fs::{self, File},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use sysinfo::{Pid, System};

pub struct PmProcess {
    pub config: PmProcessConfig,
}

impl PmProcess {
    pub fn new(cfg: PmProcessConfig) -> Self {
        PmProcess { config: cfg }
    }

    pub fn awake(self: &Self) {
        let filename_abs = self
            .config
            .exec_dir_absolute_path
            .clone()
            .join(&self.config.exec_name);
        let filename = filename_abs.as_path();

        let pm3_home_dir = pm3_safe_dir::pm3_home_dir_safe();

        let logs_dir = pm3_home_dir.join("processes").join(&self.config.exec_name);
        fs::create_dir_all(&logs_dir)
            .expect("Pm3 couldn't create folder for {self.config.exec_name}");

        let stdout_file = logs_dir.clone().join("cfg1.log");
        let stderr_file = logs_dir.clone().join("cfg1.err.log");

        let stdout = File::create(&stdout_file).expect("stdout file");
        let stderr = File::create(&stderr_file).expect("stderr file");

        let mut child = Command::new(filename)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("failed to start test_main");

        let pid = Pid::from_u32(child.id());

        let mut sys = System::new();

        println!("Started {filename:?} with PID={}", child.id());

        loop {
            if let Ok(Some(status)) = child.try_wait() {
                println!("Process exited: {status}");
                break;
            }

            sys.refresh_process(pid);

            if let Some(proc) = sys.process(pid) {
                let mem_mb = proc.memory() as f64 / 1024.0;
                let cpu = proc.cpu_usage();

                println!("[monitor] CPU: {:.2}% | RAM: {:.2} MB", cpu, mem_mb);
            } else {
                println!("Process not found");
                break;
            }

            thread::sleep(Duration::from_secs(1));
        }
    }
}
