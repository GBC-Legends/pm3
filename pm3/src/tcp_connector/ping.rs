use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};

pub fn ping_server() -> Result<String> {
    let mut stream = crate::tcp_connector::init_stream()?;

    if let Err(e) = stream.write_all(b"PING\r\n") {
        return Err(Error::new(
            e.kind(),
            format!("pm3: failed to send PING to daemon: {}", e),
        ));
    }

    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();

    match reader.read_line(&mut response) {
        Ok(0) => Err(Error::new(
            ErrorKind::UnexpectedEof,
            "pm3: daemon closed connection without responding",
        )),
        Ok(_) => Ok(format!("Response from daemon: {}", response.trim())),
        Err(e) => Err(Error::new(
            e.kind(),
            format!("pm3: failed to read response from daemon: {}", e),
        )),
    }
}
