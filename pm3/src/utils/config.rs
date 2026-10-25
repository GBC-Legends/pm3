use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use std::{fs, io};

pub struct Config {
    pub port: u16,
    pub key: String,
    pub api_address: std::net::SocketAddr,
}

impl Config {
    pub fn load() -> io::Result<Self> {
        let path = pm3_home_dir_safe().join("config.proc");
        let content = fs::read_to_string(&path).map_err(|e| io::Error::new(e.kind(), format!(
            "cannot read {}: {e}. Check the system service (systemctl status pm3@<instance>); use sudo or the service account to control it", path.display()
        )))?;
        let mut port = None;
        let mut token = None;
        let mut api_port = 8096;
        let mut api_ip: std::net::IpAddr = std::net::Ipv4Addr::LOCALHOST.into();
        for line in content.lines() {
            if let Some((key, value)) = line.trim().split_once('=') {
                match key.trim() {
                    "port" => port = value.trim().parse::<u16>().ok().filter(|p| *p > 0),
                    "token" => token = Some(value.trim().to_owned()),
                    "api_port" => {
                        api_port = value.trim().parse().map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid API port")
                        })?
                    }
                    "api_ip" => {
                        api_ip = value.trim().parse().map_err(|_| {
                            io::Error::new(io::ErrorKind::InvalidData, "invalid API address")
                        })?
                    }
                    _ => {}
                }
            }
        }
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid daemon discovery file: {}", path.display()),
            )
        };
        let config = Self {
            port: port.ok_or_else(invalid)?,
            key: token.ok_or_else(invalid)?,
            api_address: std::net::SocketAddr::new(api_ip, api_port),
        };
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        if URL_SAFE_NO_PAD.decode(&config.key).map(|k| k.len()).ok() != Some(32) {
            return Err(invalid());
        }
        Ok(config)
    }

    pub fn key(&self) -> [u8; 32] {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD
            .decode(&self.key)
            .expect("validated key")
            .try_into()
            .expect("validated key length")
    }
}
