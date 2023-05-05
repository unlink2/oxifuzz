use crate::core::{
    error::{Error, FResult},
    rand::Rand,
    transform::{Context, Word},
};
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{replace_fuzz, CommandRunnerKind};

#[derive(Clone, Default)]
pub enum Signature {
    HmacSha256 {
        secret: Word,
    },
    #[default]
    None,
}

impl Signature {
    pub fn sign(&self, data: &String) -> Option<String> {
        match self {
            Signature::HmacSha256 { secret } => {
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(&secret).expect("");
                mac.update(data.as_bytes());

                Some(general_purpose::STANDARD_NO_PAD.encode(mac.finalize().into_bytes()))
            }
            Signature::None => Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct Jwt {
    pub header: String,
    pub signature: Signature,
    pub cmd_arg_target: String,
}

pub fn jwt_command_runner(
    ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
    rand: &mut Rand,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Jwt(jwt) = &runner {
        let encoded_header = general_purpose::STANDARD_NO_PAD.encode(&replace_fuzz(
            &jwt.header,
            &jwt.cmd_arg_target,
            ctx,
            rand,
        )?);
        let encoded_payload = general_purpose::STANDARD_NO_PAD.encode(data);

        let encoded_without_signature = format!("{encoded_header}.{encoded_payload}");

        let signature = jwt.signature.sign(&encoded_without_signature);

        let encoded_final = if let Some(signature) = signature {
            format!("{encoded_without_signature}.{signature}")
        } else {
            encoded_without_signature
        };

        Ok((None, encoded_final.as_bytes().iter().map(|x| *x).collect()))
    } else {
        Err(Error::UnsupportedCommandRunner)
    }
}
