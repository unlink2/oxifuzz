use std::io::Read;

use crate::core::{
    config::{Config, SignatureConfig},
    error::{Error, FResult},
    rand::Rand,
    transform::{Context, Word},
};
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use sha2::Sha256;

use super::{replace_fuzz, CommandRunnerKind};

/// The signature algorithm used to sign a JWT token
#[derive(Clone, Default)]
pub enum Signature {
    HmacSha256 {
        secret: Word,
    },
    Rs256 {
        key_pair: Word,
    },
    #[default]
    None,
}

impl Signature {
    /// Generate a signature from a config struct
    pub fn from_config(cfg: &Config) -> FResult<Self> {
        let jwt_secret = if let Some(secret) = &cfg.jwt_secret {
            Some(secret.to_owned())
        } else if let Some(path) = &cfg.jwt_secret_file {
            let mut f = std::fs::File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            Some(buffer)
        } else {
            None
        };

        match cfg.jwt_signature {
            SignatureConfig::HmacSha256 => {
                if let Some(secret) = jwt_secret {
                    Ok(Signature::HmacSha256 { secret })
                } else {
                    Err(Error::InsufficientRunnerConfiguration)
                }
            }
            SignatureConfig::None => Ok(Signature::None),
            SignatureConfig::Rs256 => {
                if let Some(secret) = jwt_secret {
                    Ok(Signature::Rs256 { key_pair: secret })
                } else {
                    Err(Error::InsufficientRunnerConfiguration)
                }
            }
        }
    }

    /// sign input data using the signature algorithm
    pub fn sign(&self, data: &String) -> FResult<Option<String>> {
        match self {
            Signature::HmacSha256 { secret } => {
                type HmacSha256 = Hmac<Sha256>;
                let mut mac = HmacSha256::new_from_slice(&secret)?;
                mac.update(data.as_bytes());

                Ok(Some(
                    general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()),
                ))
            }
            Signature::None => Ok(Default::default()),
            Signature::Rs256 { key_pair } => {
                let key_pair = PKey::private_key_from_pem(key_pair)?;
                let mut signer = Signer::new(MessageDigest::sha256(), &key_pair)?;
                signer.update(data.as_bytes())?;
                let signature = signer.sign_to_vec()?;
                Ok(Some(general_purpose::URL_SAFE_NO_PAD.encode(signature)))
            }
        }
    }
}

/// The jwt token including the header, a signature and a fuzzing keyword
#[derive(Clone)]
pub struct Jwt {
    pub header: String,
    pub signature: Signature,
    pub cmd_arg_target: String,
}

/// The command runner for jwt tokens
/// It applies fuzzing to the header, encodes the payload and data as base64 and signs the data
/// Currently it does not support dry runs!
pub fn jwt_command_runner(
    ctx: &Context,
    runner: &CommandRunnerKind,
    data: &Word,
    rand: &mut Rand,
) -> FResult<(Option<i32>, Word)> {
    if let CommandRunnerKind::Jwt(jwt) = &runner {
        let encoded_header = general_purpose::URL_SAFE_NO_PAD.encode(&replace_fuzz(
            &jwt.header,
            &jwt.cmd_arg_target,
            ctx,
            rand,
        )?);
        let encoded_payload = general_purpose::URL_SAFE_NO_PAD.encode(data);

        let encoded_without_signature = format!("{encoded_header}.{encoded_payload}");

        let signature = jwt.signature.sign(&encoded_without_signature)?;

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
