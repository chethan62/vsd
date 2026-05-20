use thiserror::Error;

pub type Result<T> = std::result::Result<T, DecryptError>;

#[derive(Debug, Error)]
pub enum DecryptError {
    #[error("invalid mp4 format: {0}")]
    InvalidFormat(String),
    
    #[error("invalid hex string: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    
    #[error("invalid key size: expected 16 bytes got {0} bytes.")]
    InvalidKeySize(usize),
    
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
