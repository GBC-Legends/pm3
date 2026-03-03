use crate::models::process::{PmProcessStatusInfo, ProcessStatus};
use crate::utils::table::print_process_table;

use std::io::Result;

pub fn request_status() -> Result<()> {
    let reply = crate::tcp_connector::send_secure_command("list")?;

    let mut lines = reply.lines();

    let first = lines.next().unwrap_or("");
    let mut count = 0usize;

    if first.starts_with("OK ") {
        count = first[3..].trim().parse().unwrap_or(0);
    }

    let mut processes = Vec::with_capacity(count);

    for line in lines {
        if let Some(p) = parse_process(line.as_bytes()) {
            processes.push(p);
        }
    }

    print_process_table(&processes);

    Ok(())
}

fn parse_process(line: &[u8]) -> Option<PmProcessStatusInfo> {
    let mut id = 0;
    let mut name: Option<String> = None;
    let mut status = ProcessStatus::Stopped;
    let mut pid = None;
    let mut uptime = None;
    let mut cpu = None;
    let mut mem = None;
    let mut user = None;
    let mut exit_code = None;

    for field in line.split(|b| *b == b'&') {
        let eq = match memchr::memchr(b'=', field) {
            Some(p) => p,
            None => continue,
        };

        let (key, value) = (&field[..eq], &field[eq + 1..]);

        match key {
            b"id" => {
                id = atoi(value);
            }
            b"name" => {
                name = Some(String::from_utf8_lossy(value).into_owned());
            }
            b"status" => {
                status = match value {
                    b"running" => ProcessStatus::Running,
                    b"exited" => ProcessStatus::Exited,
                    _ => ProcessStatus::Stopped,
                };
            }
            b"pid" => {
                pid = Some(atoi(value) as u32);
            }
            b"uptime" => {
                uptime = Some(atoi(value));
            }
            b"cpu" => {
                cpu = fast_atof(value);
            }
            b"mem" => {
                mem = Some(atoi(value));
            }
            b"user" => {
                user = Some(String::from_utf8_lossy(value).into_owned());
            }
            b"exit_code" => {
                exit_code = Some(atoi(value) as i32);
            }
            _ => {}
        }
    }

    Some(PmProcessStatusInfo {
        id,
        name: name?,
        status,
        pid,
        uptime,
        cpu,
        mem,
        user,
        exit_code,
    })
}

#[inline]
fn atoi(bytes: &[u8]) -> u64 {
    let mut n = 0u64;
    for &b in bytes {
        if b < b'0' || b > b'9' {
            break;
        }
        n = n * 10 + (b - b'0') as u64;
    }
    n
}

#[inline]
fn fast_atof(bytes: &[u8]) -> Option<f32> {
    std::str::from_utf8(bytes).ok()?.parse().ok()
}
