use crate::models::pm3_config::PmProcessConfig;
use std::ffi::OsStr;
use std::fs;
use std::fs::create_dir_all;
use std::io;
use std::path::PathBuf;

pub(crate) fn safely_retreive_configs() -> PathBuf {
    use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
    let configs_path = pm3_home_dir_safe().join("configs");

    create_dir_all(&configs_path).expect("PM3-daemon couldn't create its .pm3/configs/ directory");

    configs_path
}

pub(crate) fn parse_configs() -> io::Result<Vec<PmProcessConfig>> {
    let mut res = Vec::new();

    for entry in fs::read_dir(safely_retreive_configs())? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if path.extension() != Some(OsStr::new("proc")) {
            continue;
        }

        let text = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let cfg: PmProcessConfig = match text.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        res.push(cfg);
    }

    Ok(res)
}
