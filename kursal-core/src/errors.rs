use thiserror::Error;

pub type Result<T> = std::result::Result<T, KursalError>;

#[derive(Error, Debug)]
pub enum KursalError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("identity error: {0}")]
    Identity(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Misc(#[from] anyhow::Error),
}

pub trait MapKursalResult<T> {
    fn ok_kursal(self, variant: impl Fn(String) -> KursalError) -> Result<T>;
}

impl<T, E: std::fmt::Display> MapKursalResult<T> for std::result::Result<T, E> {
    fn ok_kursal(self, variant: impl Fn(String) -> KursalError) -> Result<T> {
        self.map_err(|e| variant(e.to_string()))
    }
}

impl Into<String> for KursalError {
    fn into(self) -> String {
        self.to_string()
    }
}
