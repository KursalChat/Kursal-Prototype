use std::str::FromStr;

use crate::{
    KursalError, Result,
    contacts::Contact,
    crypto::stream::{derive_stream_key, stream_encrypt},
    first_contact::{FileTransferMessage, WireMessage},
    messaging::enums::MessageId,
    network::swarm::{FILE_CHUNK_SIZE, SwarmCommand, str_to_multiaddr},
};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::AsyncReadExt,
    sync::{mpsc, oneshot},
};

#[derive(Serialize, Deserialize)]
pub struct FileTransferEntry {
    pub path: String,
    pub my_random: [u8; 32],
}
impl FileTransferEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

#[derive(Serialize, Deserialize)]
pub struct FileIncomingEntry {
    pub their_random: [u8; 32],
    pub file_size: u64,
}
impl FileIncomingEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

#[derive(Serialize, Deserialize)]
pub struct FileReceiveEntry {
    pub key: [u8; 32],
    pub file_size: u64,
    pub save_path: String,
    pub received_chunks: Vec<u8>,
}
impl FileReceiveEntry {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

pub async fn send_file_chunks(
    contact: Contact,
    offer_id: MessageId,
    file_path: String,
    my_random: [u8; 32],
    their_random: [u8; 32],
    cmd_tx: mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();
    cmd_tx
        .send(SwarmCommand::OpenStream {
            peer_id: PeerId::from_str(&contact.peer_id)
                .map_err(|err| KursalError::Network(err.to_string()))?,
            addresses: str_to_multiaddr(&contact.known_addresses)?,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let stream_tx = reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?
        .ok_or_else(|| KursalError::Network("Could not open stream to peer".to_string()))?;

    //

    let mut file = File::open(&file_path).await.map_err(KursalError::Io)?;

    let mut index: u32 = 0;
    let mut buffer = vec![0u8; FILE_CHUNK_SIZE];

    let key = derive_stream_key(my_random, their_random);

    loop {
        let bytes_read = file.read(&mut buffer).await.map_err(KursalError::Io)?;

        if bytes_read == 0 {
            break;
        }

        let content = stream_encrypt(&key, &buffer[..bytes_read])?;

        let wire = WireMessage::FileTransfer(FileTransferMessage {
            transfer_id: offer_id.0,
            index,
            data: content,
        });

        let data =
            bincode::serialize(&wire).map_err(|err| KursalError::Storage(err.to_string()))?;

        stream_tx
            .send(data)
            .await
            .map_err(|err| KursalError::Network(err.to_string()))?;

        index += 1;
    }

    Ok(())
}
