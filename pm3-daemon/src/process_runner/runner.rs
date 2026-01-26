use std::{
    fs::File,
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use sysinfo::{Pid, System};

pub struct ProcessRunner {}

impl ProcessRunner {
    pub async fn run() {
        let filename = "../test_main";
        let stdout_file = format!("../{filename}-1.log");
        let stderr_file = format!("../{filename}-1.err.log");

        let stdout = File::create(&stdout_file).expect("stdout file");
        let stderr = File::create(&stderr_file).expect("stderr file");

        let mut child = Command::new(format!("./{filename}"))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("failed to start test_main");

        let pid = Pid::from_u32(child.id());

        let mut sys = System::new();

        println!("Started {filename} with PID={}", child.id());

        loop {
            // если процесс завершился — выходим
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
