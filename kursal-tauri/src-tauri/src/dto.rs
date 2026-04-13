use kursal_core::{
    contacts::Contact,
    first_contact::nearby::NearbyBeacon,
    messaging::{StoredMessage, enums::KursalMessage},
};
use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContactResponse {
    pub user_id: String,
    pub display_name: String,
    pub avatar_bytes: Option<Vec<u8>>,
    pub peer_id: String,
    pub known_addresses: Vec<String>,
    pub verified: bool,
    pub profile_shared: bool,
    pub blocked: bool,
    pub created_at: u64,
}

impl From<Contact> for ContactResponse {
    fn from(value: Contact) -> Self {
        Self {
            user_id: hex::encode(value.user_id.0),
            display_name: value.display_name,
            avatar_bytes: value.avatar_bytes,
            peer_id: value.peer_id,
            known_addresses: value.known_addresses,
            verified: value.verified,
            profile_shared: value.profile_shared,
            blocked: value.blocked,
            created_at: value.created_at,
        }
    }
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReactionResponse {
    pub emoji: String,
    pub user_id: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileDetailsDto {
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub id: String,
    pub contact_id: String,
    pub direction: String,
    pub content: String,
    pub status: String,
    pub timestamp: u64,
    pub reply_to: Option<String>,
    pub edited: bool,
    pub reactions: Vec<ReactionResponse>,
    pub file_details: Option<FileDetailsDto>,
}

impl From<StoredMessage> for MessageResponse {
    fn from(value: StoredMessage) -> Self {
        let reply_to = match &value.payload {
            KursalMessage::Text(t) => t.reply_to.map(|id| hex::encode(id.0)),
            _ => None,
        };

        let db_reactions: Vec<ReactionResponse> = value
            .reactions
            .into_iter()
            .map(|r| ReactionResponse {
                emoji: r.emoji,
                user_id: hex::encode(r.user_id.0),
            })
            .collect();

        let file_details = match &value.payload {
            KursalMessage::FileOffer(f) => Some(FileDetailsDto {
                filename: f.filename.clone(),
                size_bytes: f.size_bytes,
            }),
            _ => None,
        };

        Self {
            id: hex::encode(value.id.0),
            contact_id: hex::encode(value.contact_id.0),
            direction: match value.direction {
                kursal_core::messaging::enums::Direction::Received => "received".to_string(),
                kursal_core::messaging::enums::Direction::Sent => "sent".to_string(),
            },
            content: match &value.payload {
                KursalMessage::Text(t) => t.content.clone(),
                KursalMessage::ReactionAdd(r) => format!("Reacted {} to a message", r.emoji),
                KursalMessage::ReactionRemove(r) => format!("Removed reaction {}", r.emoji),
                KursalMessage::MessageEdit(e) => format!("Edited {}", e.new_content),
                KursalMessage::MessageDelete(_) => String::new(),
                KursalMessage::FileOffer(f) => format!("File: {}", f.filename),
                KursalMessage::FileAccept(_) => "[file offer reply]".to_string(),
                KursalMessage::CallSignal(_) => "[call]".to_string(),
                KursalMessage::DeliveryReceipt(_) => "[receipt]".to_string(),
                KursalMessage::ProfileUpdate(_) => "[profile updated]".to_string(),
            },
            status: match value.status {
                kursal_core::messaging::enums::MessageStatus::Delivered => "delivered".to_string(),
                kursal_core::messaging::enums::MessageStatus::Failed => "failed".to_string(),
                kursal_core::messaging::enums::MessageStatus::Sending => "sending".to_string(),
            },
            timestamp: value.timestamp,
            reply_to,
            edited: value.edited,
            reactions: db_reactions,
            file_details,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OtpResponse {
    pub otp: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NearbyPeerResponse {
    pub peer_id: String,
    pub session_name: String,
}

impl From<NearbyBeacon> for NearbyPeerResponse {
    fn from(value: NearbyBeacon) -> Self {
        Self {
            peer_id: value.peer_id,
            session_name: value.session_name,
        }
    }
}
