use crate::utils::start_helpers;
use std::io::{Read, Result, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct NewProcessConfig {
    pub proc_name: String,
    pub exec_dir: PathBuf,
    pub exec_name: String,
    pub exec_args: Vec<String>,
    pub active: bool,
}

impl NewProcessConfig {
    pub fn new(
        process_name: String,
        exec_dir: PathBuf,
        exec_name: String,
        exec_args: Vec<String>,
    ) -> Self {
        Self {
            proc_name: process_name,
            exec_dir,
            exec_name,
            exec_args,
            active: true,
        }
    }

    pub fn to_url_encoded(&self) -> String {
        fn enc(s: &str) -> String {
            let mut out = String::with_capacity(s.len() + s.len() / 2);
            for &b in s.as_bytes() {
                let ok = matches!(b,
                    b'A'..=b'Z' |
                    b'a'..=b'z' |
                    b'0'..=b'9' |
                    b'-' | b'.' | b'_' | b'~'
                );
                if ok {
                    out.push(b as char);
                } else {
                    out.push('%');
                    out.push_str(&format!("{:02X}", b));
                }
            }
            out
        }

        fn push_kv(dst: &mut String, key: &str, val: &str) {
            if !dst.is_empty() {
                dst.push('&');
            }
            dst.push_str(key);
            dst.push('=');
            dst.push_str(&enc(val));
        }

        let mut q = String::new();

        push_kv(&mut q, "proc_name", &self.proc_name);
        push_kv(&mut q, "exec_dir", &self.exec_dir.to_string_lossy());
        push_kv(&mut q, "exec_name", &self.exec_name);
        push_kv(&mut q, "active", if self.active { "1" } else { "0" });

        if !self.exec_args.is_empty() {
            let joined = self.exec_args.join(" ");
            push_kv(&mut q, "args", &joined);
        }

        q
    }
}

pub fn start_program(
    program: String,
    args: Vec<String>,
    interpreter: Option<String>,
    name: Option<String>,
) -> Result<String> {
    let path = Path::new(&program);

    let (exec_name, exec_args) = start_helpers::process_inputs(&interpreter, path, args, &program);

    let proc_name = match name {
        Some(name) => name,
        None => path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| program.clone()),
    };

    let exec_dir = match std::env::current_dir() {
        Ok(c) => c,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to get current directory",
            ));
        }
    };

    let new_process = NewProcessConfig::new(proc_name, exec_dir, exec_name, exec_args);

    let msg = format!("start {:?}\n", new_process.to_url_encoded());
    println!("{msg}");

    let mut stream = crate::tcp_connector::init_stream()?;

    if stream.write_all(msg.as_bytes()).is_err() {
        println!("Error: Failed to send start command");
    }

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    Ok(response)
}
