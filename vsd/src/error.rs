use reqwest::StatusCode;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    DownloadInterrupted,

    MissingKeys(String),

    UnsupportedEncryption(String),

    FfmpegFailed {
        code: i32,
        message: String,
    },

    RequestFailed {
        url: String,
        status: StatusCode,
        body: String,
    },

    CookieParse(String),

    DashParse(String),

    Mp4Parse(vsd_mp4::Error),

    Other(String),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DownloadInterrupted => write!(f, "Download interrupted due to Ctrl+C."),
            Error::MissingKeys(s) => write!(f, "Missing decryption key(s) for kid(s): {s}"),
            Error::UnsupportedEncryption(s) => write!(
                f,
                "Unsupported encryption method: {s}. Use --no-decrypt flag to download encrypted streams."
            ),
            Error::FfmpegFailed { code, message } => {
                write!(f, "Failed to execute ffmpeg ({code}): {message}")
            }
            Error::RequestFailed { url, status, body } => {
                write!(f, "Failed to request {url} ({status}): {body}")
            }
            Error::CookieParse(s) => write!(f, "Failed to parse netscape cookie: {s}."),
            Error::DashParse(s) => write!(f, "Failed to resolve dash addressing: {s}"),
            Error::Mp4Parse(e) => write!(f, "vsd-mp4: {e}"),
            Error::Other(s) => write!(f, "{s}"),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::RequestFailed {
            url: e.url().map(|x| x.as_str()).unwrap_or("unknown").to_owned(),
            status: e.status().unwrap_or(StatusCode::default()),
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

impl From<vsd_mp4::decrypt::DecryptError> for Error {
    fn from(e: vsd_mp4::decrypt::DecryptError) -> Self {
        Self::Mp4Parse(vsd_mp4::Error::from(e))
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Self::Other(e)
    }
}

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
