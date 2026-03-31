use crate::tcp_connector::send_secure_command;
use std::io::Result;

pub fn ping_server() -> Result<String> {
    let reply = send_secure_command("ping")?;
    Ok(format!("Response from daemon: {}", reply))
}
