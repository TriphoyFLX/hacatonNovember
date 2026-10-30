use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("vision: {0}")]
    Vision(String),
    #[error("input: {0}")]
    Input(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("rejected: {0}")]
    Rejected(String),
}

impl From<serde_json::Error> for CoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Protocol(value.to_string())
    }
}
