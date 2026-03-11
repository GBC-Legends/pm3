use crate::tcp_connector::{AAD, init_stream};
use crate::utils::config::Config;
use crate::utils::encryption::encrypt_reply_to_token;

use std::io::{BufRead, BufReader, ErrorKind, Result, Write};

pub fn request_logs(lines: u64, programs: Vec<String>) -> Result<()> {
    let config = Config::load();
    let key = config.key();

    let mut stream = init_stream()?;

    let mut cmd = format!("logs --lines={}", lines);

    if !programs.is_empty() {
        cmd.push(' ');
        cmd.push_str(&programs.join(" "));
    }

    let token = encrypt_reply_to_token(&key, cmd.as_bytes(), AAD);
    let out_line = format!("ENC {}\n", token);

    stream.write_all(out_line.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);

    loop {
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => {
                eprintln!("pm3: daemon closed log stream");
                break;
            }

            Ok(_) => {
                let mut msg = line.trim();

                if let Some(rest) = msg.strip_prefix("OK ") {
                    msg = rest;
                }

                if msg == "EOF" {
                    break;
                }

                println!("{}", line.trim());
            }

            Err(e) => match e.kind() {
                ErrorKind::TimedOut | ErrorKind::WouldBlock => continue,
                _ => return Err(e),
            },
        }
    }

    Ok(())
}
