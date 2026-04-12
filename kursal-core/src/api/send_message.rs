use crate::{
    KursalError, Result,
    contacts::Contact,
    crypto::messages::message_send,
    first_contact::WireMessage,
    messaging::{
        StoredMessage, StoredReaction,
        enums::{Direction, KursalMessage, MessageId, MessageStatus},
    },
    network::swarm::{SwarmCommand, str_to_multiaddr},
    storage::{SharedDatabase, get_local_user_id, get_timestamp_secs},
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use std::str::FromStr;
use tokio::sync::mpsc;

pub async fn send_message(
    content: KursalMessage,
    contact: &Contact,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<Option<MessageId>> {
    let serialized = content.serialize()?;
    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DeviceId::new(1u8).unwrap());

    let now = get_timestamp_secs()?;
    let message = message_send(db.clone(), &address, &serialized).await?;
    let wire = WireMessage::Encrypted(message.clone());

    cmd_tx
        .send(SwarmCommand::SendMessage {
            peer_id: PeerId::from_str(&contact.peer_id)
                .map_err(|err| KursalError::Storage(err.to_string()))?,
            data: bincode::serialize(&wire).map_err(|e| KursalError::Storage(e.to_string()))?,
            addresses: str_to_multiaddr(&contact.known_addresses)?,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    //

    if let KursalMessage::MessageDelete(msg) = content {
        StoredMessage::delete(&*db.0.lock().await, &contact.user_id, &msg.target_id)?;
        return Ok(None);
    }

    if let KursalMessage::MessageEdit(msg) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &msg.target_id);

        if let Ok(Some(mut message)) = loaded {
            if let KursalMessage::Text(ref mut t) = message.payload {
                t.content = msg.new_content;
            }
            message.edited = true;
            let _ = message.save(&*db.0.lock().await);
        }
        return Ok(None);
    }

    if let KursalMessage::ReactionAdd(r) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &r.target_id);

        if let Ok(Some(mut message)) = loaded {
            message.reactions.push(StoredReaction {
                emoji: r.emoji,
                user_id: get_local_user_id(&*db.0.lock().await)?,
                timestamp: now,
            });
            let _ = message.save(&*db.0.lock().await);
        }
        return Ok(None);
    }

    if let KursalMessage::ReactionRemove(r) = content {
        let loaded = StoredMessage::load(&*db.0.lock().await, &contact.user_id, &r.target_id);

        if let Ok(Some(mut message)) = loaded {
            let local_user_id = get_local_user_id(&*db.0.lock().await)?;
            message
                .reactions
                .retain(|rx| !(rx.emoji == r.emoji && rx.user_id == local_user_id));
            let _ = message.save(&*db.0.lock().await);
        }
        return Ok(None);
    }

    let Some(msg_id) = content.message_id() else {
        return Ok(None);
    };

    // dont save some kind of messages (some that returns the message_id but should not save)
    if matches!(
        content,
        KursalMessage::DeliveryReceipt(_)
            | KursalMessage::FileAccept(_)
            | KursalMessage::CallSignal(_)
    ) {
        return Ok(Some(msg_id));
    }

    let stored = StoredMessage {
        id: msg_id,
        status: MessageStatus::Sending,
        direction: Direction::Sent,
        timestamp: now,
        contact_id: contact.user_id.clone(),
        payload: content,
        raw_ciphertext: Some(message),
        edited: false,
        reactions: Vec::with_capacity(0),
    };

    stored.save(&*db.0.lock().await)?;

    Ok(Some(msg_id))
}
