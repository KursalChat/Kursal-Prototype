use crate::{KursalError, Result};
use serde::{Deserialize, Serialize};
use uuid::{NoContext, Timestamp};

#[derive(Serialize, Deserialize)]
pub enum Direction {
    Sent,
    Received,
}

#[derive(Serialize, Deserialize)]
pub enum MessageStatus {
    Sending,
    Delivered,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy)]
pub struct MessageId(pub [u8; 16]);
#[allow(clippy::new_without_default)]
impl MessageId {
    pub fn new() -> Self {
        MessageId(uuid::Uuid::new_v7(Timestamp::now(NoContext)).into_bytes())
    }
}

#[derive(Serialize, Deserialize)]
pub enum KursalMessage {
    Text(TextMessage),
    ReactionAdd(ReactionAdd),
    ReactionRemove(ReactionRemove),
    MessageEdit(MessageEdit),
    MessageDelete(MessageDelete),
    FileOffer(FileOffer),
    FileAccept(FileAccept),
    CallSignal(CallSignal),
    DeliveryReceipt(DeliveryReceipt),
    ProfileUpdate(ProfileInfo),
}

impl KursalMessage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }

    pub fn message_id(&self) -> Option<MessageId> {
        match self {
            KursalMessage::Text(m) => Some(m.id),
            KursalMessage::FileOffer(m) => Some(m.id),
            KursalMessage::CallSignal(m) => Some(m.call_id),
            KursalMessage::DeliveryReceipt(m) => Some(m.message_id),
            KursalMessage::FileAccept(_) => None,
            KursalMessage::ReactionAdd(_) => None,
            KursalMessage::ReactionRemove(_) => None,
            KursalMessage::MessageEdit(_) => None,
            KursalMessage::MessageDelete(_) => None,
            KursalMessage::ProfileUpdate(_) => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TextMessage {
    pub id: MessageId,
    pub content: String,
    pub timestamp: u64,
    pub reply_to: Option<MessageId>,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionAdd {
    pub target_id: MessageId,
    pub emoji: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionRemove {
    pub target_id: MessageId,
    pub emoji: String,
}

#[derive(Serialize, Deserialize)]
pub struct MessageEdit {
    pub target_id: MessageId,
    pub new_content: String,
    pub edited_at: u64,
}

#[derive(Serialize, Deserialize)]
pub struct MessageDelete {
    pub target_id: MessageId,
}

#[derive(Serialize, Deserialize)]
pub struct FileOffer {
    pub id: MessageId,
    pub filename: String,
    pub size_bytes: u64,
    pub random: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct FileAccept {
    pub offer_id: MessageId,
    pub random: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub enum CallSignalKind {
    Offer,
    Answer,
    IceCandidate,
    Hangup,
}

#[derive(Serialize, Deserialize)]
pub struct CallSignal {
    pub call_id: MessageId,
    pub kind: CallSignalKind,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub message_id: MessageId,
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct KeyRotation {
    pub old_peer_id: String,
    pub new_peer_id: String,
    pub new_addresses: Vec<String>,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

pub const MAX_PROFILE_AVATAR_LEN: usize = 128 * 1000; // 128 KB
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileInfo {
    pub display_name: String,
    pub avatar_bytes: Option<Vec<u8>>,
}
impl ProfileInfo {
    pub fn validate(&self) -> Result<()> {
        if let Some(avatar) = &self.avatar_bytes
            && avatar.len() > MAX_PROFILE_AVATAR_LEN
        {
            return Err(KursalError::Identity(
                "Profile avatar exceeds maximum allowed size.".to_string(),
            ));
        }

        if self.display_name.len() < 3 || self.display_name.len() > 32 {
            return Err(KursalError::Identity(
                "Profile display name must be between 3 and 32 characters.".to_string(),
            ));
        }

        // TODO: maybe allow better rules :p
        if !self.display_name.is_ascii() {
            return Err(KursalError::Identity(
                "Profile display name must only contain ascii characters.".to_string(),
            ));
        };

        Ok(())
    }
}
