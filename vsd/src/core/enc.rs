use crate::error::Result;
use std::sync::Arc;
use vsd_mp4::decrypt::{CencDecryptor, HlsAes128Decrypter, HlsSampleAesDecrypter};

#[derive(Clone)]
pub enum Decrypter {
    Aes128(HlsAes128Decrypter),
    Cenc(Arc<CencDecryptor>),
    SampleAes(HlsSampleAesDecrypter),
    None,
}

impl Decrypter {
    pub fn is_hls(&self) -> bool {
        matches!(self, Decrypter::Aes128(_) | Decrypter::SampleAes(_))
    }

    pub fn increment_iv(&mut self) {
        match self {
            Decrypter::Aes128(processor) => processor.increment_iv(),
            Decrypter::SampleAes(processor) => processor.increment_iv(),
            _ => (),
        }
    }

    pub fn decrypt(&self, input: Vec<u8>, init: Option<&[u8]>) -> Result<Vec<u8>> {
        Ok(match self {
            Decrypter::Cenc(processor) => processor.decrypt(input, init)?,
            Decrypter::Aes128(processor) => processor.decrypt(input),
            Decrypter::SampleAes(processor) => processor.decrypt(input),
            Decrypter::None => input,
        })
    }
}
