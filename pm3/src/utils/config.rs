use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use std::{fs, io, net::SocketAddr};

pub struct Config {
    pub port: u16,
    pub key: String,
    pub api_address: SocketAddr,
}

impl Config {
    pub fn load() -> io::Result<Self> {
        let path = pm3_home_dir_safe().join("config.proc");
        let content = fs::read_to_string(&path).map_err(|e| io::Error::new(e.kind(), format!(
            "cannot read {}: {e}. Start this user's pm3-<user>.service; to control another account use sudo -H -u USER pm3", path.display()
        )))?;
        Self::parse(&content)
    }

    fn parse(content: &str) -> io::Result<Self> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid config.proc; restart this user's PM3 service",
            )
        };
        let mut port = None;
        let mut token = None;
        let mut api_address = "127.0.0.1:8096".parse().unwrap();
        for line in content.lines() {
            if let Some((name, value)) = line.split_once('=') {
                match name.trim() {
                    "port" => port = value.trim().parse::<u16>().ok().filter(|p| *p != 0),
                    "token" => token = Some(value.trim().to_owned()),
                    "api_addr" => api_address = value.trim().parse().map_err(|_| invalid())?,
                    _ => {}
                }
            }
        }
        let key = token.ok_or_else(invalid)?;
        if URL_SAFE_NO_PAD.decode(&key).map(|v| v.len()).ok() != Some(32) {
            return Err(invalid());
        }
        Ok(Self {
            port: port.ok_or_else(invalid)?,
            key,
            api_address,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    const TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    #[test]
    fn reads_each_users_api_endpoint_and_accepts_legacy_discovery() {
        let legacy = format!("port=8046\ntoken={TOKEN}");
        assert_eq!(Config::parse(&legacy).unwrap().api_address.port(), 8096);
        let other = Config::parse(&format!("{legacy}\napi_addr=[::1]:41235")).unwrap();
        assert_eq!(other.api_address.to_string(), "[::1]:41235");
    }
    #[test]
    fn corrupt_discovery_is_an_error_instead_of_a_panic() {
        for content in [
            "port=1",
            "port=1\ntoken=bad",
            &format!("port=0\ntoken={TOKEN}"),
            &format!("port=1\ntoken={TOKEN}\napi_addr=bad"),
        ] {
            assert!(Config::parse(content).is_err());
        }
    }
}
