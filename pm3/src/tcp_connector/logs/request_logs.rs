use crate::tcp_connector::{AAD, init_stream, send_secure_command};
use crate::utils::config::Config;
use crate::utils::encryption::{DecryptError, decrypt_wire_line, encrypt_reply_to_token};

use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};

pub fn request_logs(lines: Option<u64>, mut programs: Vec<String>) -> Result<()> {
    let config = Config::load();
    let key = config.key();

    if programs.is_empty() {
        let all = send_secure_command("list programs")?;
        programs = all.split_whitespace().map(|s| s.to_string()).collect();
    }

    let mut stream = init_stream()?;

    let mut cmd = String::from("logs");

    if let Some(n) = lines {
        cmd.push_str(&format!(" --lines={}", n));
    }

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
                let decrypted = match decrypt_wire_line(&key, &line, AAD) {
                    Ok(d) => d,
                    Err(e) => {
                        match e {
                            DecryptError::BadBase64(e) => {
                                eprintln!("pm3: invalid base64 from daemon: {}", e);
                            }
                            DecryptError::TooShort => {
                                eprintln!("pm3: daemon log message too short");
                            }
                            DecryptError::BadVersion(v) => {
                                eprintln!("pm3: unsupported encryption version: {}", v);
                            }
                            DecryptError::Crypto => {
                                eprintln!("pm3: failed to authenticate log message");
                            }
                        }

                        return Err(Error::new(ErrorKind::InvalidData, "log decryption failed"));
                    }
                };

                let mut msg = String::from_utf8_lossy(&decrypted).trim().to_string();

                if let Some(rest) = msg.strip_prefix("OK ") {
                    msg = rest.to_string();
                }

                if msg == "EOF" {
                    break;
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
