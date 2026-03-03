use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};

use crate::utils::config::Config;
use crate::utils::encryption::{decrypt_wire_line, encrypt_reply_to_token};

const AAD: &[u8] = b"pm3:tcp:v1";

pub fn ping_server() -> Result<String> {
    let config = Config::load();
    let key = config.key();

    let mut stream = crate::tcp_connector::init_stream()?;

    let token = encrypt_reply_to_token(&key, b"ping", AAD);
    let out_line = format!("ENC {}\n", token);

    if let Err(e) = stream.write_all(out_line.as_bytes()) {
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
        Ok(_) => {
            let decrypted = decrypt_wire_line(&key, &response, AAD)
                .map_err(|_| Error::new(ErrorKind::Other, "decryption failed"))?;

            let response = String::from_utf8_lossy(&decrypted);

            Ok(format!("Response from daemon: {}", response.trim()))
        }
        Err(e) => Err(Error::new(
            e.kind(),
            format!("pm3: failed to read response from daemon: {}", e),
        )),
    }
}
