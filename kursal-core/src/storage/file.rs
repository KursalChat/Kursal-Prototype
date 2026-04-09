use crate::{KursalError, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum KursalFile {
    LtcPayload(Vec<u8>),
}

impl KursalFile {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn get_warning(&self) -> String {
        match self {
            KursalFile::LtcPayload(_) => {
                "This file contains a long term code. Opening it will add a new contact."
                    .to_string()
            }
        }
    }
}
