use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub(crate) struct PmProcessConfig {
    pub proc_name: String,
    pub exec_dir: PathBuf,
    pub exec_name: PathBuf,
    pub exec_args: Vec<String>,
    pub active: bool,

    pub _extra: HashMap<String, String>,
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
            exec_dir: PathBuf::from(exec_dir),
            exec_name: PathBuf::from(exec_name),
            exec_args,
            active,
            _extra: map,
        })
    }
}

impl PmProcessConfig {
    pub fn dump(&self) -> String {
        let mut out = String::with_capacity(256);

        out.push_str("proc_name=");
        out.push_str(&self.proc_name);
        out.push('\n');

        out.push_str("exec_dir=");
        out.push_str(&self.exec_dir.to_string_lossy());
        out.push('\n');

        out.push_str("exec_name=");
        out.push_str(&self.exec_name.to_string_lossy());
        out.push('\n');

        out.push_str("exec_args=");
        if !self.exec_args.is_empty() {
            out.push_str(&self.exec_args.join(" "));
        }
        out.push('\n');

        out.push_str("active=");
        out.push_str(if self.active { "1" } else { "0" });
        out.push('\n');

        out
    }
}
