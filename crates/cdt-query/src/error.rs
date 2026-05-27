use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("api error: {0}")]
    Api(String),
}

impl From<cdt_api::ApiError> for QueryError {
    fn from(e: cdt_api::ApiError) -> Self {
        Self::Api(e.to_string())
    }
}
