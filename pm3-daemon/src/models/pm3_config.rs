use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub(crate) struct PmProcessConfig {
    pub proc_name: String,
    pub exec_dir_absolute_path: PathBuf,
    pub exec_name: String,
    pub exec_args: Vec<String>,
    pub active: bool,

    pub extra: HashMap<String, String>,
}

impl FromStr for PmProcessConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut map = HashMap::<String, String>::new();

        for line in s.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| format!("Invalid line: {line}"))?;

            map.insert(k.trim().to_string(), v.trim().to_string());
        }

        let proc_name = map.remove("proc_name").ok_or("missing proc_name")?;

        let exec_dir = map.remove("exec_dir").ok_or("missing exec_dir")?;

        let exec_name = map.remove("exec_name").ok_or("missing exec_name")?;

        let exec_args_string = map.remove("exec_args").unwrap_or_default();
        let exec_args: Vec<String> = exec_args_string
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let active = map
            .remove("active")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);

        Ok(PmProcessConfig {
            proc_name,
            exec_dir_absolute_path: PathBuf::from(exec_dir),
            exec_name,
            exec_args,
            active,
            extra: map,
        })
    }
}
