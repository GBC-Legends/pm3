use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};

const ENC_V1: u8 = 1;
const NONCE_LEN: usize = 12;

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
