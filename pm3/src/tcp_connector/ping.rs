use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

pub fn ping_server() {
    let address = "127.0.0.1:8046";

    // connect
    let mut stream = match TcpStream::connect(address) {
        Ok(s) => {
            s
        }
        Err(_) => {
            println!("pm3: could not connect to daemon");
            return;
        }
    };

    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    // send ping
    if stream.write_all(b"PING\r\n").is_err() {
        println!("pm3: failed to send ping to daemon");
        return;
    }

    stream.flush().unwrap();

    // read response line
    let mut reader = BufReader::new(stream);
    let mut response = String::new();

    match reader.read_line(&mut response) {
        Ok(_) => {
            println!("Response from daemon: {}", response.trim());
        }
        Err(_) => println!("pm3: daemon did not respond"),
    }
}