use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("invalid headers: {0}")]
    InvalidHeaders(#[from] reqwest::header::InvalidHeaderValue),

    #[error("cannot construct client due to previous error in initialization")]
    CannotConstructHttpClient,

    #[error("cannot login: {0}")]
    CannotLogin(String),

    #[error("could not compose the query: {0}")]
    QuerySerialization(#[from] serde_json::Error),

    #[error("could not deserialize response: {0}")]
    Deserialization(#[from] csv::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
