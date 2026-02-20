pub mod ping;
pub mod start;
pub mod stop;

use std::io::Result;
use std::net::TcpStream;
use std::time::Duration;

pub fn init_stream() -> Result<TcpStream> {
    let address = "127.0.0.1:8046";

    let stream = match TcpStream::connect(address) {
        Ok(s) => s,
        Err(_) => {
            println!("pm3: could not connect to daemon");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to connect",
            ));
        }
    };

    stream
        .set_read_timeout(Some(Duration::from_secs(3)))?;

    Ok(stream)
}
