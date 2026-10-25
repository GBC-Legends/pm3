use crate::tcp_connector::{AAD, init_stream, send_secure_command};
use crate::utils::config::Config;
use crate::utils::encryption::{decrypt_wire_line, encrypt_reply_to_token};
use std::collections::HashMap;

use crossterm::style::{Color, Stylize};
use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};

pub fn request_logs(lines: Option<u64>, mut programs: Vec<String>) -> Result<()> {
    let config = Config::load();
    let key = config.key();
    let process_names = get_process_names()?;

    if programs.iter().any(|p| p == "all") {
        if programs.len() > 1 {
            eprintln!(
                "{}",
                "pm3: warning: 'all' overrides other program arguments".with(Color::Red)
            );

            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        programs.clear();
    }

    if programs.is_empty() {
        let all = send_secure_command("list-programs")?;
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

                    if parts.len() < 3 {
                        continue;
                    }

                    let pid = parts[0];
                    let stream = parts[1];
                    let text = parts[2].trim();

                    if text.is_empty() {
                        continue;
                    }

                    let name = process_names.get(pid).map(|s| s.as_str()).unwrap_or("?");

                    match stream {
                        "stdout" => {
                            println!(
                                "[{}:{}] {}",
                                pid.with(Color::Blue),
                                name.with(Color::Cyan),
                                text.with(Color::Green)
                            );
                        }
                        "stderr" => {
                            println!(
                                "[{}:{}] {}",
                                pid.with(Color::Blue),
                                name.with(Color::Cyan),
                                text.with(Color::Red)
                            );
                        }
                        _ => {}
                    }

                    continue;
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

fn get_process_names() -> Result<HashMap<String, String>> {
    let mut names = HashMap::new();

    let reply = send_secure_command("list-programs")?;

    let clean = if let Some(rest) = reply.strip_prefix("OK ") {
        rest
    } else {
        &reply
    };

    let parts: Vec<&str> = clean.split_whitespace().collect();

    let mut i = 0;

    while i + 1 < parts.len() {
        names.insert(parts[i].to_string(), parts[i + 1].to_string());
        i += 2;
    }

    Ok(names)
}
