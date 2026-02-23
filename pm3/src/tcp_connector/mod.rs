pub mod ping;
pub mod start;
pub mod status;
pub mod stop;

use std::io::{ErrorKind, Result};
use std::net::TcpStream;
use std::time::Duration;

pub fn init_stream() -> Result<TcpStream> {
    let address = "127.0.0.1:8046";

    let stream = match TcpStream::connect(address) {
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
