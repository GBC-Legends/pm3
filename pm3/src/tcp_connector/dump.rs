use std::io::Result;

pub fn dump_program() -> Result<String> {
    let reply = crate::tcp_connector::send_secure_command("dump")?;
    Ok(reply)
}
