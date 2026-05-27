use reqwest::StatusCode;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    CookieParse(String),
    DashParse(String),
    DownloadInterrupted,
    FfmpegFailed {
        code: i32,
        message: String,
    },
    MissingKey(String),
    MissingSegments,
    Mp4Parse(vsd_mp4::Error),
    Other(String),
    RequestFailed {
        url: String,
        status: StatusCode,
        body: String,
    },
    UnsupportedEncryption(String),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DownloadInterrupted => write!(f, "Download interrupted due to Ctrl+C."),
            Self::MissingSegments => write!(f, "Stream contains no segments."),
            Self::MissingKey(x) => write!(f, "Missing decryption key for {}.", x),
            Self::UnsupportedEncryption(x) => write!(
                f,
                "Unsupported encryption method: {}. Use --no-decrypt flag to download encrypted streams.",
                x
            ),
            Self::FfmpegFailed { code, message } => {
                write!(f, "Failed to execute ffmpeg ({}): {}", code, message)
            }
            Self::RequestFailed { url, status, body } => {
                write!(f, "Failed to request {} ({}): {}", url, status, body)
            }
            Self::CookieParse(x) => write!(f, "Failed to parse netscape cookie: {}.", x),
            Self::DashParse(x) => write!(f, "Failed to resolve dash addressing: {}", x),
            Self::Mp4Parse(x) => write!(f, "vsd-mp4: {}", x),
            Self::Other(x) => write!(f, "{}", x),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::RequestFailed {
            url: e.url().map(|x| x.as_str()).unwrap_or("unknown").to_owned(),
            status: e.status().unwrap_or_default(),
            body: e.to_string(),
        }
    }
}

impl From<crate::cookie::ParseError> for Error {
    fn from(e: crate::cookie::ParseError) -> Self {
        Self::CookieParse(e.to_string())
    }
}

impl From<vsd_mp4::Error> for Error {
    fn from(e: vsd_mp4::Error) -> Self {
        Self::Mp4Parse(e)
    }
}

macro_rules! impl_from_other {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Error {
                fn from(e: $t) -> Self {
                    Self::Other(e.to_string())
                }
            }
        )*
    };
}

impl_from_other!(
    String,
    std::array::TryFromSliceError,
    std::io::Error,
    std::num::ParseIntError,
    std::string::FromUtf8Error,
    base64::DecodeError,
    serde_json::Error,
    requestty::ErrorKind,
    reqwest::header::InvalidHeaderName,
    reqwest::header::InvalidHeaderValue,
    reqwest::header::ToStrError,
    tokio::task::JoinError,
    url::ParseError
);

#[cfg(feature = "capture")]
impl_from_other!(chromiumoxide::error::CdpError);

#[cfg(feature = "license")]
impl_from_other!(playready::Error, widevine::Error);

#[macro_export]
macro_rules! bail {
    ($msg:literal $(,)?) => {
        return Err($crate::error::Error::Other($msg.into()))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::error::Error::Other(format!($fmt, $($arg)*)))
    };
}
