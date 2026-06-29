use reqwest::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("unsupported platform: {os}/{arch}")]
    UnsupportedPlatform {
        os: &'static str,
        arch: &'static str,
    },

    #[error("{context}: {source}")]
    Network {
        context: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("invalid HTTP header value: {0}")]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),

    #[error("download failed: HTTP {status} for {url}")]
    HttpStatus { status: StatusCode, url: String },

    #[error("{0}")]
    Download(String),

    #[error("{0}")]
    Archive(String),

    #[error("{0}")]
    Validation(String),
}

impl InstallError {
    pub(crate) fn network(context: impl Into<String>, source: reqwest::Error) -> Self {
        Self::Network {
            context: context.into(),
            source,
        }
    }
}
