use kursal_core::KursalError;
use serde::Serialize;

pub struct CommandError(String);

impl From<KursalError> for CommandError {
    fn from(e: KursalError) -> Self {
        Self(e.to_string())
    }
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

pub type Result<T> = std::result::Result<T, CommandError>;
