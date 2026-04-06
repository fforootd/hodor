use std::fmt;

#[derive(Debug)]
pub enum FgaError {
    BadRequest(String),
    NotFound(String),
    Forbidden(String),
    Unsupported(String),
    Internal(anyhow::Error),
}

impl fmt::Display for FgaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "{msg}"),
            Self::NotFound(msg) => write!(f, "{msg}"),
            Self::Forbidden(msg) => write!(f, "{msg}"),
            Self::Unsupported(msg) => write!(f, "{msg}"),
            Self::Internal(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for FgaError {}

impl From<anyhow::Error> for FgaError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value)
    }
}
