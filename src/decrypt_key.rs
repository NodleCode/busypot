use scrypt::{scrypt, Params};
use sodiumoxide::crypto::secretbox;

#[derive(Debug)]
pub enum Error {
    InvalidScryptParams,
    InvalidScryptOutput,
    DecryptionFailed,
}

use std::{
    error::Error as StdError,
    fmt::{Display, Formatter, Result as fmtResult},
};
impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmtResult {
        write!(f, "{:?}", self)
    }
}

impl StdError for Error {}

pub fn decrypt(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>, Error> {
    let salt = [];
    let nonce = [0u8; secretbox::NONCEBYTES];

    let params = Params::new(15, 8, 1, 32).map_err(|_| Error::InvalidScryptParams)?;

    let mut key = [0u8; secretbox::KEYBYTES];
    scrypt(password.as_bytes(), &salt, &params, &mut key)
        .map_err(|_| Error::InvalidScryptOutput)?;

    let key = secretbox::Key(key);

    let decrypted = secretbox::open(encrypted_data, &secretbox::Nonce(nonce), &key)
        .map_err(|_| Error::DecryptionFailed)?;

    Ok(decrypted)
}
