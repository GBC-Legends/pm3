use std::io::{BufRead, BufReader, Result, Write};

pub fn ping_server() -> Result<String> {
    let mut stream = crate::tcp_connector::init_stream()?;

    if stream.write_all(b"PING\r\n").is_err() {
        println!("pm3: failed to send ping to daemon");
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to send ping to daemon",
        ));
    }

    stream.flush().unwrap();

    let mut reader = BufReader::new(stream);
    let mut response = String::new();

    match reader.read_line(&mut response) {
        Ok(_) => Ok(format!("Response from daemon: {}", response.trim())),
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Daemon did not respond",
        )),
    }
}
