use crate::models::pm3_config::PmProcessConfig;
use std::ffi::OsStr;
use std::fs;
use std::fs::create_dir_all;
use std::io;
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn safely_retreive_configs() -> PathBuf {
    use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
    let configs_path = pm3_home_dir_safe().join("configs");

    create_dir_all(&configs_path)
        .expect("PM3 daemon couldn't create its instance configs directory");

    configs_path
}

pub(crate) fn parse_configs() -> io::Result<Vec<PmProcessConfig>> {
    let mut res = Vec::new();

    let mut paths = fs::read_dir(safely_retreive_configs())?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    paths.sort();
    for path in paths {
        if path.extension() != Some(OsStr::new("proc")) {
            continue;
        }

        let text = fs::read_to_string(&path).map_err(|e| {
            io::Error::new(e.kind(), format!("cannot read {}: {e}", path.display()))
        })?;
        let cfg: PmProcessConfig = text.parse().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid process config {}: {e}", path.display()),
            )
        })?;

        res.push(cfg);
    }

    Ok(res)
}

pub(crate) fn save_config(config: &PmProcessConfig, cfg_path: &PathBuf) -> anyhow::Result<()> {
    let mut file = fs::File::create(cfg_path)?;

    let dump = config.dump();

    file.write_all(dump.as_bytes())?;

    Ok(())
}
