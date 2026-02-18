use crate::utils::start_helpers;
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

    let is_script = interpreter.is_some() || start_helpers::is_script_ext(path);

    let (exec_name, exec_args) = if is_script {
        let runner = interpreter
            .as_deref()
            .or_else(|| start_helpers::runner_for_ext(path))
            .unwrap();

        let runner_path = if start_helpers::has_path_separators(runner) {
            start_helpers::to_abs_best_effort(runner)
        } else {
            start_helpers::search_in_path(runner)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| runner.to_string())
        };

        let mut exec_args = Vec::with_capacity(1 + args.len());
        exec_args.push(program.clone());
        exec_args.extend(args);

        (runner_path, exec_args)
    } else {
        let exec = if start_helpers::has_path_separators(&program) {
            start_helpers::to_abs_best_effort(&program)
        } else {
            start_helpers::search_in_path(&program)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| program.clone())
        };

        (exec, args)
    };

    let proc_name = match name {
        Some(name) => name,
        None => path.file_stem().unwrap().to_string_lossy().to_string(),
    };

    // 4 - убрать unwrap все
    let exec_dir = env::current_dir().unwrap();

    let new_process = NewProcessConfig::new(proc_name, exec_dir, exec_name.to_string(), exec_args);

    println!("{new_process:?}");
    // 6 - вынести стрим в функцию и передать ему `format!("START {}", new_process.to_url_encoded())`
}
