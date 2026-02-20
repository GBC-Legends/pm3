use std::io::{BufRead, BufReader, Result, Write};

pub fn stop_program(
    programs: Vec<String>,
) -> Result<String> {
    let msg = format!("stop {}\n", programs.join(" "));

    let mut stream = crate::tcp_connector::init_stream()?;
    stream.write_all(msg.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    println!("resp={response}");

    Ok(response)
}