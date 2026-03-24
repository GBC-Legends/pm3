use crate::tcp_connector::{AAD, init_stream, send_secure_command};
use crate::utils::config::Config;
use crate::utils::encryption::{decrypt_wire_line, encrypt_reply_to_token};

use crossterm::style::{Color, Stylize};
use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};

pub fn request_logs(lines: Option<u64>, mut programs: Vec<String>) -> Result<()> {
    let config = Config::load();
    let key = config.key();

    if programs.is_empty() {
        let all = send_secure_command("list programs")?;
        programs = all.split_whitespace().map(|s| s.to_string()).collect();
    }

    let mut cmd = String::from("logs");

    if let Some(n) = lines {
        cmd.push_str(&format!(" --lines={}", n));
    }

    if !programs.is_empty() {
        cmd.push(' ');
        cmd.push_str(&programs.join(" "));
    }

    let mut stream = init_stream()?;

    let token = encrypt_reply_to_token(&key, cmd.as_bytes(), AAD);
    let out_line = format!("ENC {}\n", token);

    stream.write_all(out_line.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);

    loop {
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => break,

            Ok(_) => {
                let decrypted = decrypt_wire_line(&key, &line, AAD)
                    .map_err(|_| Error::new(ErrorKind::InvalidData, "log decryption failed"))?;

                let mut msg = String::from_utf8_lossy(&decrypted).trim().to_string();

                if let Some(rest) = msg.strip_prefix("OK ") {
                    msg = rest.to_string();
                }

                if msg == "EOF" {
                    break;
                }

                if msg == "LOGS" {
                    continue;
                }

                if let Some(rest) = msg.strip_prefix("LOG ") {
                    let parts: Vec<&str> = rest.splitn(3, ' ').collect();

                    if parts.len() == 3 {
                        let pid = parts[0];
                        let stream = parts[1];
                        let text = parts[2];

                        match stream {
                            "stdout" => {
                                println!(
                                    "[{}][{}] {}",
                                    pid.with(Color::Blue),
                                    "out".with(Color::Green),
                                    text
                                );
                            }
                            "stderr" => {
                                println!(
                                    "[{}][{}] {}",
                                    pid.with(Color::Blue),
                                    "err".with(Color::Red),
                                    text
                                );
                            }
                            _ => {
                                println!("{msg}");
                            }
                        }

                        continue;
                    }
                }

                println!("{msg}");
            }

            Err(e) => match e.kind() {
                ErrorKind::TimedOut | ErrorKind::WouldBlock => continue,
                _ => return Err(e),
            },
        }
    }

    Ok(())
}
