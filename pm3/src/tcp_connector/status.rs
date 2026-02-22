use std::io::{BufRead, BufReader, ErrorKind, Result, Write};
use crate::models::process::{PmProcessStatusInfo, ProcessStatus};
use crate::tcp_connector::init_stream;
use crate::utils::table::print_process_table;

pub fn request_status() -> Result<()> {
    let mut stream = init_stream()?;

    stream.write_all(b"LIST\r\n")?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut buffer = String::new();

    loop {
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => buffer.push_str(&line),
            Err(e) if e.kind() == ErrorKind::WouldBlock
                || e.kind() == ErrorKind::TimedOut =>
                {
                    break;
                }

            Err(e) => return Err(e),
        }
    }

    let mut processes: Vec<PmProcessStatusInfo> = Vec::new();

    for line in buffer.lines() {

        if line.trim().is_empty() {
            continue;
        }

        let mut id: u64 = 0;
        let mut name = String::new();
        let mut status = ProcessStatus::Stopped;
        let mut pid: Option<u32> = None;
        let mut uptime: Option<u64> = None;
        let mut cpu: Option<f32> = None;
        let mut mem: Option<u64> = None;
        let mut user: Option<String> = None;
        let mut exit_code: Option<i32> = None;

        for pair in line.split('&') {
            let mut parts = pair.splitn(2, '=');

            let key = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("");

            match key {
                "id" => id = value.parse().unwrap_or(0),
                "name" => name = value.to_string(),
                "status" => {
                    status = match value {
                        "running" => ProcessStatus::Running,
                        "exited" => ProcessStatus::Exited,
                        _ => ProcessStatus::Stopped,
                    }
                }
                "pid" => pid = value.parse().ok(),
                "uptime" => uptime = value.parse().ok(),
                "cpu" => cpu = value.parse().ok(),
                "mem" => mem = value.parse().ok(),
                "user" => user = Some(value.to_string()),
                "exit_code" => exit_code = value.parse().ok(),
                _ => {}
            }
        }

        if !name.is_empty() {
            processes.push(PmProcessStatusInfo {
                id,
                name,
                status,
                pid,
                uptime,
                cpu,
                mem,
                user,
                exit_code,
            });
        }
    }

    print_process_table(&processes);
    Ok(())
}