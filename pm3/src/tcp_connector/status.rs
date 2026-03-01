use crate::models::process::{PmProcessStatusInfo, ProcessStatus};
use crate::tcp_connector::init_stream;
use crate::utils::table::print_process_table;

use std::io::{BufRead, BufReader, Result, Write};

pub fn request_status() -> Result<()> {
    let stream = init_stream()?;

    let mut reader = BufReader::with_capacity(64 * 1024, stream);

    reader.get_mut().write_all(b"LIST\r\n")?;

    let mut count = 0usize;

    read_frame_line(&mut reader, |line| {
        if !line.starts_with(b"OK ") {
            return;
        }
        count = atoi(&line[3..]) as usize;
    })?;

    let mut processes = Vec::with_capacity(count);

    for _ in 0..count {
        read_frame_line(&mut reader, |line| {
            if let Some(p) = parse_process(line) {
                processes.push(p);
            }
        })?;
    }

    print_process_table(&processes);

    Ok(())
}

fn read_frame_line<R, F>(reader: &mut R, mut f: F) -> Result<()>
where
    R: BufRead,
    F: FnMut(&[u8]),
{
    loop {
        let buf = reader.fill_buf()?;

        if buf.is_empty() {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }

        if let Some(pos) = memchr::memchr(b'\n', buf) {
            let mut line = &buf[..pos];

            if line.ends_with(b"\r") {
                line = &line[..line.len() - 1];
            }

            f(line);
            reader.consume(pos + 1);
            return Ok(());
        }

        let len = buf.len();
        reader.consume(len);
    }
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
