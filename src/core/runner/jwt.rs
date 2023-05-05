use crate::core::transform::Word;
use hmac::{Hmac, Mac};
use sha2::Sha256;

#[derive(Clone, Default)]
pub enum Signature {
    HmacSha256 {
        secret: Word,
    },
    #[default]
    None,
}

impl Signature {
    pub fn sign(&self, data: &Word) -> Word {
        match self {
            Signature::HmacSha256 { secret } => {
                type HmacSha256 = Hmac<Sha256>;
                let mac = HmacSha256::new_from_slice(&secret).expect("");
                mac.finalize().into_bytes().iter().map(|x| *x).collect()
            }
            Signature::None => data.to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct Jwt {
    header: String,
    payload: String,
    cmd_arg_target: String,
}
