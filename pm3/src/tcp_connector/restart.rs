use std::io::{Error, ErrorKind, Result};

use crossterm::style::{Color, Stylize};

use crate::tcp_connector::send_secure_command;

pub fn restart_program(mut programs: Vec<String>) -> Result<String> {
    if programs.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "no programs specified"));
    }

    if programs.iter().any(|p| p == "all") && programs.len() > 1 {
        eprintln!(
            "{}",
            "pm3: warning: 'all' overrides other program arguments".with(Color::Red)
        );

        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let reply = send_secure_command("list-programs")?;
    let clean = reply.strip_prefix("OK ").unwrap_or(&reply);

    let mut resolved = Vec::new();
    let mut parts = clean.split_whitespace();

    while let Some(id) = parts.next() {
        if let Some(name) = parts.next() {
            if programs.iter().any(|p| p == "all") {
                resolved.push(id.to_string());
            } else if programs.iter().any(|p| p == id || p == name) {
                resolved.push(id.to_string());
            }
        }
    }

    programs = resolved;

    if programs.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "no programs found"));
    }

    let plaintext = format!("restart {}", programs.join(" "));
    let reply = send_secure_command(&plaintext)?;

    Ok(reply)
}
