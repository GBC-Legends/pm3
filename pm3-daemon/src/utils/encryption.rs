use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};

const ENC_V1: u8 = 1;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

pub fn encrypt_reply_to_token(key32: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> String {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key32));

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let nonce = Nonce::from_slice(&nonce_bytes);

    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("encrypt should not fail");

    let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
    out.push(ENC_V1);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);

    URL_SAFE_NO_PAD.encode(out)
}

#[derive(Debug)]
pub enum DecryptError {
    BadBase64(base64::DecodeError),
    TooShort,
    BadVersion(u8),
    Crypto,
}

impl From<base64::DecodeError> for DecryptError {
    fn from(e: base64::DecodeError) -> Self {
        Self::BadBase64(e)
    }
}

pub fn decrypt_token_to_reply(
    key32: &[u8; 32],
    token_b64: &str,
    aad: &[u8],
) -> Result<Vec<u8>, DecryptError> {
    let raw = URL_SAFE_NO_PAD.decode(token_b64)?;

    if raw.len() < 1 + NONCE_LEN + TAG_LEN {
        return Err(DecryptError::TooShort);
    }

    let ver = raw[0];
    if ver != ENC_V1 {
        return Err(DecryptError::BadVersion(ver));
    }

    let nonce_bytes: [u8; NONCE_LEN] = raw[1..1 + NONCE_LEN]
        .try_into()
        .map_err(|_| DecryptError::TooShort)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ct = &raw[1 + NONCE_LEN..];

    let cipher = ChaCha20Poly1305::new(Key::from_slice(key32));
    cipher
        .decrypt(nonce, Payload { msg: ct, aad })
        .map_err(|_| DecryptError::Crypto)
}

pub fn decrypt_wire_line(
    key32: &[u8; 32],
    line: &str,
    aad: &[u8],
) -> Result<Vec<u8>, DecryptError> {
    let s = line.trim();
    let token = s.strip_prefix("ENC ").unwrap_or(s);
    decrypt_token_to_reply(key32, token, aad)
}
