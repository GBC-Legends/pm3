use std::io::Result;

pub fn revive_program() -> Result<String> {
    let reply = crate::tcp_connector::send_secure_command("revive")?;
    Ok(reply)
}
