use reqwest::StatusCode;

pub type Result<T> = std::result::Result<T, Error>;

/// Early-return with [`Error::Other`]. Accepts the same arguments as [`format!`].
#[macro_export]
macro_rules! bail {
    ($msg:literal $(,)?) => {
        return Err($crate::error::Error::Other($msg.into()))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::error::Error::Other(format!($fmt, $($arg)*)))
    };
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{url} request failed ({status}): {body}")]
    RequestFailed {
        url: String,
        status: StatusCode,
        body: String,
    },

    #[error("{0}")]
    DashAddressing(String),

    #[error("Missing content decryption keys for KIDs: {0}")]
    MissingKeys(String),

    #[error(
        "Unsupported encryption method: {0}. Use --no-decrypt flag to download encrypted streams."
    )]
    UnsupportedEncryption(String),

    #[error("Download interrupted due to Ctrl+C.")]
    DownloadInterrupted,

    #[error("Failed to execute ffmpeg ({code}): {message}")]
    FfmpegFailed { code: i32, message: String },

    // --- Third-party ---
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    UrlParse(#[from] url::ParseError),

    #[error(transparent)]
    Mp4(#[from] vsd_mp4::Error),

    #[error(transparent)]
    Base64Decode(#[from] base64::DecodeError),

    #[error(transparent)]
    HeaderToStr(#[from] reqwest::header::ToStrError),

    #[error(transparent)]
    Prompt(#[from] requestty::ErrorKind),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    #[error(transparent)]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

    #[error("{0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::Other(e)
    }
}

impl From<std::array::TryFromSliceError> for Error {
    fn from(e: std::array::TryFromSliceError) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<vsd_mp4::decrypt::DecryptError> for Error {
    fn from(e: vsd_mp4::decrypt::DecryptError) -> Self {
        Error::Mp4(vsd_mp4::Error::from(e))
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        Error::Other(e.to_string())
    }
}

impl From<crate::cookie::ParseError> for Error {
    fn from(e: crate::cookie::ParseError) -> Self {
        Error::Other(e.to_string())
    }
}

#[cfg(feature = "capture")]
impl From<chromiumoxide::error::CdpError> for Error {
    fn from(e: chromiumoxide::error::CdpError) -> Self {
        Error::Other(e.to_string())
    }
}

#[cfg(feature = "license")]
impl From<playready::Error> for Error {
    fn from(e: playready::Error) -> Self {
        Error::Other(e.to_string())
    }
}

#[cfg(feature = "license")]
impl From<widevine::Error> for Error {
    fn from(e: widevine::Error) -> Self {
        Error::Other(e.to_string())
    }
}
