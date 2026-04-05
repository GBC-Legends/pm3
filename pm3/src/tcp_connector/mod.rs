pub mod delete;
pub mod logs;
pub mod monitor;
pub mod ping;
pub mod restart;
pub mod start;
pub mod status;
pub mod stop;

use crate::utils::config::Config;
use crate::utils::encryption::DecryptError;
use crate::utils::encryption::{decrypt_wire_line, encrypt_reply_to_token};
use std::io::{BufRead, BufReader, Error, ErrorKind, Result, Write};
use std::net::TcpStream;
use std::time::Duration;

pub const AAD: &[u8] = b"pm3:tcp:v1";

pub fn send_secure_command(cmd: &str) -> Result<String> {
    let config = Config::load();
    let key = config.key();

    let mut stream = init_stream()?;

    let token = encrypt_reply_to_token(&key, cmd.as_bytes(), AAD);
    let out_line = format!("ENC {}\n", token);

    if let Err(e) = stream.write_all(out_line.as_bytes()) {
        match e.kind() {
            ErrorKind::BrokenPipe => {
                eprintln!("pm3: daemon closed connection (broken pipe)");
            }
            ErrorKind::ConnectionReset => {
                eprintln!("pm3: connection reset by daemon");
            }
            _ => {
                eprintln!("pm3: failed to send command to daemon: {}", e);
            }
        }
        return Err(e);
    }

    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();

    match reader.read_line(&mut response) {
        Ok(0) => {
            eprintln!("pm3: daemon closed connection without responding");
            Err(Error::new(
                ErrorKind::UnexpectedEof,
                "daemon closed connection",
            ))
        }
        Ok(_) => {
            let decrypted = match decrypt_wire_line(&key, &response, AAD) {
                Ok(d) => d,
                Err(e) => {
                    match e {
                        DecryptError::BadBase64(e) => {
                            eprintln!("pm3: daemon response is not valid base64: {}", e);
                        }
                        DecryptError::TooShort => {
                            eprintln!("pm3: daemon response is too short");
                        }
                        DecryptError::BadVersion(v) => {
                            eprintln!("pm3: unsupported encryption version: {}", v);
                        }
                        DecryptError::Crypto => {
                            eprintln!("pm3: failed to authenticate daemon response");
                        }
                    }

                    return Err(Error::new(ErrorKind::InvalidData, "decryption failed"));
                }
            };

            Ok(String::from_utf8_lossy(&decrypted).trim().to_string())
        }
        Err(e) => {
            match e.kind() {
                ErrorKind::TimedOut => {
                    eprintln!("pm3: daemon response timed out");
                }
                ErrorKind::ConnectionReset => {
                    eprintln!("pm3: connection reset while reading response");
                }
                _ => {
                    eprintln!("pm3: failed to read response from daemon: {}", e);
                }
            }
            Err(e)
        }
    }
}

pub fn init_stream() -> Result<TcpStream> {
    let config = Config::load();
    let address = format!("127.0.0.1:{}", config.port);

    let stream = match TcpStream::connect(&address) {
        Ok(s) => s,
        Err(e) => {
            match e.kind() {
                ErrorKind::ConnectionRefused => {
                    eprintln!("pm3: daemon is not running (connection refused)");
                }
                ErrorKind::TimedOut => {
                    eprintln!("pm3: connection to daemon timed out");
                }
                ErrorKind::AddrNotAvailable => {
                    eprintln!("pm3: invalid daemon address {}", address);
                }
                _ => {
                    eprintln!("pm3: failed to connect to daemon: {}", e);
                }
            }

            return Err(e);
        }
    };

    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(3))) {
        eprintln!("pm3: failed to configure socket timeout: {}", e);
        return Err(e);
    }

    Ok(stream)
}
