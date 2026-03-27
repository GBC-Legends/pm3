use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use anyhow::{self, Context};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore};
use std::fs::OpenOptions;
use std::io::Write;
use std::net::{SocketAddr, TcpListener};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const DEFAULT_PORT: u16 = 8046;
const DEFAULT_METRICS_API_PORT: u16 = 8096;

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub port: u16,
    pub key_b64: String,
    pub metrics_api_port: u16,
    pub metrics_api_addr: String,
}

impl DaemonConfig {
    pub fn new() -> anyhow::Result<Self> {
        let port = Self::find_available_port_with_default(DEFAULT_PORT)?;

        // is expected to be stable for proper reverse_proxy
        // to set up later with proper SSL and TLS support and auth in the way suits your needs better.
        // we do not want you to be forced to stick to JWT, Bearer, SSO, Oauth2.
        // you can select level of security you have to use that suits your needs better.
        let metrics_api_port = DEFAULT_METRICS_API_PORT;

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        let result = Self {
            port,
            key_b64: URL_SAFE_NO_PAD.encode(key),
            metrics_api_port,
            metrics_api_addr: "127.0.0.1".to_string(),
        };

        result.dump()?;

        Ok(result)
    }

    fn find_available_port_with_default(default_port: u16) -> anyhow::Result<u16> {
        if Self::is_port_free(default_port) {
            return Ok(default_port);
        }

        let listener =
            TcpListener::bind("127.0.0.1:0").context("failed to bind to ephemeral port")?;

        let addr: SocketAddr = listener.local_addr()?;
        Ok(addr.port())
    }

    fn is_port_free(port: u16) -> bool {
        TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    fn dump(&self) -> anyhow::Result<()> {
        let pm3_home_dir = pm3_home_dir_safe();
        std::fs::create_dir_all(&pm3_home_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&pm3_home_dir, std::fs::Permissions::from_mode(0o700))?;
        }

        let cfg_path = pm3_home_dir.join("config.proc");

        #[cfg(unix)]
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&cfg_path)?;

        #[cfg(windows)]
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&cfg_path)?;

        write!(file, "port={}\ntoken={}", self.port, self.key_b64)?;

        Ok(())
    }

    pub fn key(&self) -> [u8; 32] {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.key_b64)
            .expect("invalid base64 key");

        bytes.try_into().expect("key must be 32 bytes")
    }
}
