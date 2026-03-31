use crate::utils::pm3_safe_dir::pm3_home_dir_safe;
use std::fs;

pub struct Config {
    pub port: u16,
    pub key: String,
}

impl Config {
    pub fn load() -> Self {
        let mut path = pm3_home_dir_safe();
        path.push("config.proc");

        let content = fs::read_to_string(path).expect("cannot read config.proc");

        let mut port = 0;
        let mut token = String::new();

        for line in content.lines() {
            let line = line.trim();

            if let Some((key, value)) = line.split_once('=') {
                match key.trim() {
                    "port" => {
                        port = value.trim().parse().expect("invalid port");
                    }
                    "token" => {
                        token = value.trim().to_string();
                    }
                    _ => {}
                }
            }
        }

        Self { port, key: token }
    }

    pub fn key(&self) -> [u8; 32] {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

        let bytes = URL_SAFE_NO_PAD
            .decode(&self.key)
            .expect("invalid base64 key");

        bytes.try_into().expect("key must be 32 bytes")
    }
}
