use crate::utils::start_helpers;
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
        // 5 - написать безопасный энкодер
        todo!("FIDAN RABOTAI")
    }
}

pub fn start_program(
    program: String,
    args: Vec<String>,
    interpreter: Option<String>,
    name: Option<String>,
) {
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
        Err(_) => return,
    };

    let new_process = NewProcessConfig::new(proc_name, exec_dir, exec_name, exec_args);

    println!("{new_process:?}");
    // 6 - вынести стрим в функцию и передать ему `format!("START {}", new_process.to_url_encoded())`
}
