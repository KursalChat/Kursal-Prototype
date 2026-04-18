use crate::{
    Result,
    contacts::Contact,
    identity::UserId,
    messaging::{StoredMessage, enums::MessageId},
};
use libsignal_protocol::{DeviceId, ProtocolAddress};
use tokio::sync::oneshot;

pub mod cmd_wrapper;
pub mod file_transfers;
pub mod handle_core_command;
pub mod handle_incoming;
pub mod send_message;
pub mod state;

pub use handle_core_command::handle_core_command;
pub use handle_incoming::handle_incoming;
pub use send_message::send_message;

pub enum ConnectionStatus {
    Connecting,
    Relay,
    HolePunch,
    Direct,
    Disconnected,
}

pub enum AppEvent {
    MessageReceived {
        contact_id: UserId,
        message: StoredMessage,
    },
    TypingIndicator {
        contact_id: UserId,
    },
    DeliveryConfirmed {
        contact_id: UserId,
        message_id: MessageId,
    },
    MessageEdited {
        contact_id: UserId,
        message_id: MessageId,
        new_content: String,
    },
    MessageDeleted {
        contact_id: UserId,
        message_id: MessageId,
    },
    ReactionAdded {
        contact_id: UserId,
        message_id: MessageId,
        emoji: String,
    },
    ReactionRemoved {
        contact_id: UserId,
        message_id: MessageId,
        emoji: String,
    },
    ContactAdded {
        contact: Contact,
    },
    ContactUpdated {
        contact: Contact,
    },
    ContactRemoved {
        contact_id: UserId,
    },
    PeerIdRotated {
        new_addresses: Vec<String>,
    },
    ConnectionChange {
        contact_id: UserId,
        status: ConnectionStatus,
    },
    LTCExpiringSoon {
        hours_remaining: u32,
    },
    NearbyRequest {
        peer_id: String,
        session_name: String,
        decision_tx: oneshot::Sender<bool>,
    },
    FileOffered {
        contact_id: UserId,
        offer_id: MessageId,
        filename: String,
        size_bytes: u64,
    },
    FileTransferProgress {
        transfer_id: MessageId,
        bytes_transferred: u64,
        total_bytes: u64,
    },
    FileReceived {
        contact_id: UserId,
        save_path: String,
    },
}

pub enum CoreCommand {
    PublishOtp {
        otp: String,
        reply: oneshot::Sender<Result<()>>,
    },
    FetchOtp {
        otp: String,
        reply: oneshot::Sender<Result<Contact>>,
    },
    ExportLtc {
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    ImportLtc {
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<Contact>>,
    },
    ConnectNearby {
        peer_id: String,
        session_name: String,
        method: String,
        reply: oneshot::Sender<Result<()>>,
    },
    SendText {
        contact_id: String,
        text: String,
        reply_to: Option<MessageId>,
        reply: oneshot::Sender<Result<MessageId>>,
    },
    SendTypingIndicator {
        contact_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    RotatePeerId {
        reply: oneshot::Sender<Result<()>>,
    },
    RemoveContact {
        contact_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    DeleteMessage {
        contact_id: String,
        message_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    EditMessage {
        contact_id: String,
        message_id: String,
        new_content: String,
        reply: oneshot::Sender<Result<()>>,
    },
    ReactionAdd {
        contact_id: String,
        message_id: String,
        emoji: String,
        reply: oneshot::Sender<Result<()>>,
    },
    ReactionRemove {
        contact_id: String,
        message_id: String,
        emoji: String,
        reply: oneshot::Sender<Result<()>>,
    },
    DeleteLocalMessage {
        contact_id: String,
        message_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    ShareProfile {
        contact_id: String,
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<()>>,
    },
    BroadcastProfile {
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<()>>,
    },
    SendFileOffer {
        contact_id: String,
        file_path: String,
        reply: oneshot::Sender<Result<(MessageId, u64)>>,
    },
    AcceptFileOffer {
        contact_id: String,
        offer_id: String,
        save_path: String,
        reply: oneshot::Sender<Result<()>>,
    },
}

pub fn get_protocol_addr(user_id: [u8; 32]) -> ProtocolAddress {
    ProtocolAddress::new(hex::encode(user_id), DeviceId::new(1u8).unwrap())
}
