use std::path::{Component, Path, PathBuf};
use std::{env, fs};

pub fn has_path_separators(s: &str) -> bool {
    s.contains('/') || s.contains('\\')
}

#[cfg(unix)]
fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(p)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable_file(p: &Path) -> bool {
    fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

#[cfg(windows)]
fn pathext_list() -> Vec<String> {
    let raw = env::var("PATHEXT").unwrap_or(".EXE;.CMD;.BAT;.COM".to_string());
    raw.split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

pub fn search_in_path(cmd: &str) -> Option<PathBuf> {
    if has_path_separators(cmd) {
        let p = PathBuf::from(cmd);
        if is_executable_file(&p) || p.is_file() {
            return Some(p);
        }
        return None;
    }

    let path_var = env::var_os("PATH")?;
    let paths = env::split_paths(&path_var);

    #[cfg(windows)]
    {
        let exts = pathext_list();
        for dir in paths {
            let candidate = dir.join(cmd);
            if candidate.is_file() {
                return Some(candidate);
            }

            for ext in &exts {
                let candidate = dir.join(format!("{cmd}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    #[cfg(unix)]
    {
        for dir in paths {
            let candidate = dir.join(cmd);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
        None
    }
}

pub fn is_script_ext(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };

    match ext.to_ascii_lowercase().as_str() {
        "py" | "pyw" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "sh" | "bash" | "zsh" | "fish"
        | "bat" | "cmd" | "ps1" | "rb" | "php" | "pl" | "lua" | "r" | "groovy" | "kts" | "dart"
        | "swift" => true,
        _ => false,
    }
}

pub fn to_abs_best_effort(program: &str) -> String {
    let cwd = match env::current_dir() {
        Ok(c) => c,
        Err(_) => return program.to_string(),
    };

    let joined = cwd.join(program);

    if let Ok(p) = fs::canonicalize(&joined) {
        return p.to_string_lossy().to_string();
    }

    normalize_path(&joined).to_string_lossy().to_string()
}

pub fn runner_for_ext(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();

    const TABLE: &[(&str, &str)] = &[
        ("py", "python3"),
        ("pyw", "python3"),
        ("js", "node"),
        ("mjs", "node"),
        ("cjs", "node"),
        ("ts", "ts-node"),
        ("tsx", "ts-node"),
        ("sh", "bash"),
        ("bash", "bash"),
        ("zsh", "zsh"),
        ("fish", "fish"),
        ("bat", "cmd"),
        ("cmd", "cmd"),
        ("ps1", "powershell"),
        ("rb", "ruby"),
        ("php", "php"),
        ("pl", "perl"),
        ("lua", "lua"),
    ];

    TABLE
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, runner)| *runner)
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = out.pop();
                if !popped {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }

    out
}

pub fn process_inputs(
    interpreter: &Option<String>,
    path: &Path,
    args: Vec<String>,
    program: &str,
) -> (String, Vec<String>) {
    let is_script = interpreter.is_some() || is_script_ext(path);

    if is_script {
        let runner = interpreter
            .as_deref()
            .or_else(|| runner_for_ext(path))
            .unwrap();

        let runner_path = if has_path_separators(runner) {
            to_abs_best_effort(runner)
        } else {
            search_in_path(runner)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| runner.to_string())
        };

        let mut exec_args = Vec::with_capacity(1 + args.len());
        exec_args.push(program.to_string());
        exec_args.extend(args);

        (runner_path, exec_args)
    } else {
        let exec = if has_path_separators(&program) {
            to_abs_best_effort(&program)
        } else {
            search_in_path(&program)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| program.to_string())
        };

        (exec, args)
    }
}
