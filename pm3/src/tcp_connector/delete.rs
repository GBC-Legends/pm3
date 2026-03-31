use std::io::{Error, ErrorKind, Result};

use crate::tcp_connector::send_secure_command;

pub fn delete_program(programs: Vec<String>) -> Result<String> {
    if programs.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "no programs specified"));
    }

    let mut targets = Vec::new();

    for arg in programs {
        if arg == "all" {
            let reply = send_secure_command("list-programs")?;
            let mut parts = reply.split_whitespace();

            while let Some(id) = parts.next() {
                let id = id.to_string();

                if !targets.contains(&id) {
                    targets.push(id);
                }

                parts.next();
            }
        } else {
            targets.push(arg);
        }
    }

    if targets.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "no programs found"));
    }

    let plaintext = format!("delete {}", targets.join(" "));
    let reply = send_secure_command(&plaintext)?;

    Ok(reply)
}
