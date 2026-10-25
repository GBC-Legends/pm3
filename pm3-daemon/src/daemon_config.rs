use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    net::{IpAddr, SocketAddr, TcpListener},
    sync::OnceLock,
};

// dotenv defaults for managed applications, without mutating the environment of
// a multithreaded Tokio runtime. Existing process environment takes precedence.
static PROCESS_ENV: OnceLock<HashMap<String, String>> = OnceLock::new();
pub(crate) fn process_environment() -> &'static HashMap<String, String> {
    PROCESS_ENV.get_or_init(HashMap::new)
}

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub port: u16,
    pub key_b64: String,
    pub metrics_api_port: u16,
    pub metrics_api_addr: String,
}

impl DaemonConfig {
    pub fn new() -> anyhow::Result<(Self, TcpListener, TcpListener)> {
        let home = pm3_home_dir_safe();
        fs::create_dir_all(home.join("configs"))?;
        let env_path = home.join("pm3.env");
        let mut settings = HashMap::new();
        if env_path.exists() {
            for entry in dotenvy::from_path_iter(&env_path)
                .with_context(|| format!("cannot read {}", env_path.display()))?
            {
                let (name, value) = entry?;
                settings.insert(name, value);
            }
        }
        let value = |name: &str| {
            std::env::var(name)
                .ok()
                .or_else(|| settings.get(name).cloned())
        };
        let ip: IpAddr = value("PM3_IP")
            .unwrap_or_else(|| "127.0.0.1".into())
            .parse()
            .context("invalid PM3_IP")?;
        let port = |name: &str| -> anyhow::Result<Option<u16>> {
            value(name)
                .map(|v| v.parse().with_context(|| format!("invalid {name}")))
                .transpose()
        };
        // Reserve listeners instead of probing then dropping them. Different
        // users starting simultaneously cannot accidentally select the same port.
        let cli = bind("127.0.0.1".parse().unwrap(), port("PM3_PORT")?, 8046)?;
        let api = bind(ip, port("PM3_API_PORT")?, 8096)?;
        let api_port = api.local_addr()?.port();
        if !env_path.exists() {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&env_path)?;
            writeln!(file, "PM3_IP={ip}\nPM3_API_PORT={api_port}")?;
        }
        let defaults = settings
            .into_iter()
            .filter(|(name, _)| std::env::var_os(name).is_none())
            .collect();
        PROCESS_ENV
            .set(defaults)
            .map_err(|_| anyhow::anyhow!("daemon configuration initialized twice"))?;
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let cfg = Self {
            port: cli.local_addr()?.port(),
            key_b64: URL_SAFE_NO_PAD.encode(key),
            metrics_api_addr: ip.to_string(),
            metrics_api_port: api_port,
        };
        cli.set_nonblocking(true)?;
        api.set_nonblocking(true)?;
        Ok((cfg, cli, api))
    }

    pub fn publish(&self) -> anyhow::Result<()> {
        let home = pm3_home_dir_safe();
        let path = home.join("config.proc");
        let temp = home.join("config.proc.tmp");
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
        let ip = match self.metrics_api_addr.as_str() {
            "0.0.0.0" => "127.0.0.1",
            "::" => "::1",
            ip => ip,
        };
        let api = SocketAddr::new(ip.parse()?, self.metrics_api_port);
        writeln!(
            file,
            "port={}\ntoken={}\napi_addr={api}",
            self.port, self.key_b64
        )?;
        file.sync_all()?;
        drop(file);
        fs::rename(temp, path)?;
        Ok(())
    }

    pub fn key(&self) -> [u8; 32] {
        URL_SAFE_NO_PAD
            .decode(&self.key_b64)
            .expect("generated key")
            .try_into()
            .expect("32-byte key")
    }
}

fn bind(ip: IpAddr, configured: Option<u16>, default: u16) -> anyhow::Result<TcpListener> {
    let listener = match configured {
        Some(port) => TcpListener::bind((ip, port)),
        None => TcpListener::bind((ip, default)).or_else(|_| TcpListener::bind((ip, 0))),
    };
    listener.with_context(|| {
        format!(
            "cannot bind {}",
            SocketAddr::new(ip, configured.unwrap_or(default))
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn different_users_can_reserve_distinct_ports_but_explicit_conflicts_fail() {
        let ip = "127.0.0.1".parse().unwrap();
        let first = bind(ip, Some(0), 8046).unwrap();
        let used = first.local_addr().unwrap().port();
        assert!(bind(ip, Some(used), 8046).is_err());
        let second = bind(ip, None, used).unwrap();
        assert_ne!(used, second.local_addr().unwrap().port());
    }
}
