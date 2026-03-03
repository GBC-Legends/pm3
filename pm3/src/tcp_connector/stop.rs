use std::io::Result;

pub fn stop_program(programs: Vec<String>) -> Result<String> {
    let plaintext = format!("stop {}", programs.join(" "));
    let reply = crate::tcp_connector::send_secure_command(&plaintext)?;
    Ok(reply)
}
