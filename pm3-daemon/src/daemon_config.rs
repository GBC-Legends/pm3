use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    net::TcpListener,
};

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub port: u16,
    pub key_b64: String,
    pub metrics_api_port: u16,
    pub metrics_api_addr: String,
}

impl DaemonConfig {
    // Keep both sockets bound until handed to Tokio: simultaneous instance startup
    // must not race between probing a free port and opening the actual listener.
    pub fn new() -> anyhow::Result<(Self, TcpListener, TcpListener)> {
        let home = pm3_home_dir_safe();
        fs::create_dir_all(home.join("configs"))?;
        let env_path = std::env::var_os("PM3_CONFIG")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                if cfg!(target_os = "linux") && std::env::var_os("PM3_HOME").is_none() {
                    std::path::PathBuf::from("/etc/pm3")
                        .join(format!("{}.env", pm3_runtime::instance().unwrap()))
                } else {
                    home.join("pm3.env")
                }
            });
        let mut settings = HashMap::new();
        if env_path.exists() {
            for pair in dotenvy::from_path_iter(&env_path)
                .with_context(|| format!("cannot read {}", env_path.display()))?
            {
                let (key, value) = pair?;
                settings.insert(key, value);
            }
        }
        let setting = |name: &str| {
            std::env::var(name)
                .ok()
                .or_else(|| settings.get(name).cloned())
        };
        let address = setting("PM3_IP").unwrap_or_else(|| "127.0.0.1".into());
        address
            .parse::<std::net::IpAddr>()
            .context("PM3_IP must be an IPv4 or IPv6 address")?;
        let port = |name: &str| -> anyhow::Result<Option<u16>> {
            setting(name)
                .map(|v| v.parse::<u16>().with_context(|| format!("invalid {name}")))
                .transpose()
        };
        let cli = bind_listener("127.0.0.1", port("PM3_PORT")?, 8046)?;
        let api = bind_listener(&address, port("PM3_API_PORT")?, 8096)?;
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let config = Self {
            port: cli.local_addr()?.port(),
            key_b64: URL_SAFE_NO_PAD.encode(key),
            metrics_api_port: api.local_addr()?.port(),
            metrics_api_addr: address,
        };
        cli.set_nonblocking(true)?;
        api.set_nonblocking(true)?;
        Ok((config, cli, api))
    }

    pub fn dump(&self) -> anyhow::Result<()> {
        let path = pm3_home_dir_safe().join("config.proc");
        let temp = path.with_extension("proc.tmp");
        // Remove a stale temporary file left by a crash before creating it exclusively.
        match fs::remove_file(&temp) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        let api_ip = match self.metrics_api_addr.as_str() {
            "0.0.0.0" => "127.0.0.1",
            "::" => "::1",
            ip => ip,
        };
        writeln!(
            file,
            "port={}\ntoken={}\napi_port={}\napi_ip={api_ip}",
            self.port, self.key_b64, self.metrics_api_port
        )?;
        file.sync_all()?;
        drop(file);
        fs::rename(temp, path)?;
        Ok(())
    }

    pub fn key(&self) -> [u8; 32] {
        URL_SAFE_NO_PAD
            .decode(&self.key_b64)
            .expect("invalid key")
            .try_into()
            .expect("key must be 32 bytes")
    }
}

fn bind_listener(
    address: &str,
    configured: Option<u16>,
    default: u16,
) -> anyhow::Result<TcpListener> {
    let result = match configured {
        Some(port) => TcpListener::bind((address, port)),
        None => TcpListener::bind((address, default)).or_else(|_| TcpListener::bind((address, 0))),
    };
    result.with_context(|| format!("cannot bind {address}:{}", configured.unwrap_or(default)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn occupied_explicit_ports_fail_but_automatic_ports_are_reserved() {
        let first = bind_listener("127.0.0.1", Some(0), 8046).unwrap();
        let port = first.local_addr().unwrap().port();
        assert!(bind_listener("127.0.0.1", Some(port), 8046).is_err());
        let second = bind_listener("127.0.0.1", None, port).unwrap();
        assert_ne!(port, second.local_addr().unwrap().port());
    }
}
