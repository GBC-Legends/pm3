use std::env;
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
    pub fn new(process_name: String, exec_dir: PathBuf, exec_name: String, exec_args: Vec<String>) -> Self {
        Self {
            proc_name: process_name,
            exec_dir,
            exec_name,
            exec_args,
            active: true
        }
    }

    pub fn to_url_encoded(&self) -> String {
        todo!("FIDAN RABOTAI")
    }
}

pub fn start_program(program: String, args: Vec<String>) {
    let path = Path::new(&program);

    let exec_name = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        env::current_dir().unwrap().join(path)
    };

    let proc_name = path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let exec_dir = env::current_dir().unwrap();

    let new_process = NewProcessConfig::new(proc_name, exec_dir, exec_name.to_string_lossy().to_string(), args);

    println!("{new_process:?}");
}