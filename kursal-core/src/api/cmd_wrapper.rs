use crate::{
    KursalError, Result,
    api::CoreCommand,
    contacts::Contact,
    dto::{ContactResponse, MessageResponse, NearbyPeerResponse, OtpResponse},
    first_contact::{
        nearby::{NearbyBeacon, generate_session_name},
        otp,
    },
    identity::{UserId, security_code},
    messaging::{StoredMessage, enums::MessageId},
    network::NetworkManager,
    storage::{
        Database, get_dilithium_pub, get_local_identity_pub, get_local_profile, get_local_user_id,
        set_local_profile,
    },
};
use std::collections::HashMap;
use tokio::sync::{MutexGuard, mpsc, oneshot};

pub trait StateWrapper {
    fn core_cmd_tx(&self) -> &mpsc::Sender<CoreCommand>;
    fn network_lock(&self) -> impl std::future::Future<Output = MutexGuard<'_, NetworkManager>>;
    fn pending_nearby_lock(
        &self,
    ) -> impl std::future::Future<Output = MutexGuard<'_, HashMap<String, oneshot::Sender<bool>>>>;
    fn db_lock(&self) -> impl std::future::Future<Output = MutexGuard<'_, Database>>;
}

pub async fn generate_otp() -> Result<OtpResponse> {
    Ok(OtpResponse {
        otp: otp::generate_otp()?,
    })
}

pub async fn publish_otp<S: StateWrapper>(state: S, otp: String) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::PublishOtp {
            otp,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?
}

pub async fn fetch_otp<S: StateWrapper>(state: S, otp: String) -> Result<ContactResponse> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::FetchOtp {
            otp,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let contact = reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(ContactResponse::from(contact))
}

pub async fn export_ltc<S: StateWrapper>(state: S) -> Result<Vec<u8>> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::ExportLtc { reply: reply_tx })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?
}

pub async fn import_ltc<S: StateWrapper>(state: S, bytes: Vec<u8>) -> Result<ContactResponse> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::ImportLtc {
            bytes,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let contact = reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(ContactResponse::from(contact))
}

pub async fn start_nearby<S: StateWrapper>(state: S) -> Result<String> {
    let session_name = generate_session_name()?;
    let mut network = state.network_lock().await;

    let beacon = NearbyBeacon {
        peer_id: network.primary.peer_id.to_base58(),
        session_name: session_name.clone(),
    };

    network.start_mdns(beacon).await?;

    Ok(session_name)
}

pub async fn stop_nearby<S: StateWrapper>(state: S) -> Result<()> {
    let mut network = state.network_lock().await;

    network.stop_mdns().await?;

    Ok(())
}

pub async fn get_nearby_peers<S: StateWrapper>(state: S) -> Result<Vec<NearbyPeerResponse>> {
    let network = state.network_lock().await;

    let peers = crate::network::get_nearby_peers(&network).await;

    Ok(peers
        .into_iter()
        .map(|(_, beacon, origin)| NearbyPeerResponse::from((beacon, origin)))
        .collect())
}

pub async fn connect_nearby<S: StateWrapper>(
    state: S,
    peer_id: String,
    method: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    let session_name = state
        .network_lock()
        .await
        .my_beacon
        .lock()
        .await
        .as_ref()
        .map(|b| b.session_name.clone())
        .ok_or(KursalError::Network("No active beacon".to_string()))?;

    state
        .core_cmd_tx()
        .send(CoreCommand::ConnectNearby {
            peer_id,
            session_name,
            method,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn accept_nearby<S: StateWrapper>(state: S, peer_id: String) -> Result<()> {
    if let Some(tx) = state.pending_nearby_lock().await.remove(&peer_id) {
        tx.send(true).ok();
    }

    Ok(())
}

pub async fn decline_nearby<S: StateWrapper>(state: S, peer_id: String) -> Result<()> {
    if let Some(tx) = state.pending_nearby_lock().await.remove(&peer_id) {
        tx.send(false).ok();
    }

    Ok(())
}

pub async fn get_contacts<S: StateWrapper>(state: S) -> Result<Vec<ContactResponse>> {
    let contacts = Contact::load_all(&*state.db_lock().await)?;

    Ok(contacts.into_iter().map(ContactResponse::from).collect())
}

pub async fn get_contact<S: StateWrapper>(
    state: S,
    contact_id: String,
) -> Result<Option<ContactResponse>> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    let contact = Contact::load(&*state.db_lock().await, &UserId(user_id_bytes))?;

    Ok(contact.map(ContactResponse::from))
}

pub async fn remove_contact<S: StateWrapper>(state: S, contact_id: String) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::RemoveContact {
            contact_id,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn send_text<S: StateWrapper>(
    state: S,
    contact_id: String,
    text: String,
    reply_to: Option<String>,
) -> Result<String> {
    let reply_to_id = match reply_to {
        Some(id) => {
            let bytes: [u8; 16] = hex::decode(&id)
                .map_err(|err| KursalError::Crypto(err.to_string()))?
                .try_into()
                .map_err(|_| KursalError::Crypto("Invalid reply_to message id".to_string()))?;
            Some(MessageId(bytes))
        }
        None => None,
    };

    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::SendText {
            contact_id,
            text,
            reply_to: reply_to_id,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let msg_id = reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(hex::encode(msg_id.0))
}

pub async fn send_typing_indicator<S: StateWrapper>(state: S, contact_id: String) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::SendTypingIndicator {
            contact_id,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn delete_local_message<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::DeleteLocalMessage {
            contact_id,
            message_id,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn get_messages<S: StateWrapper>(
    state: S,
    contact_id: String,
    limit: usize,
    before: Option<String>,
) -> Result<Vec<MessageResponse>> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    let before = match before {
        None => None,
        Some(id) => {
            let message_id_bytes: [u8; 16] = hex::decode(id)
                .map_err(|err| KursalError::Crypto(err.to_string()))?
                .try_into()
                .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;
            Some(MessageId(message_id_bytes))
        }
    };

    let stored = StoredMessage::load_all(
        &*state.db_lock().await,
        &UserId(user_id_bytes),
        limit,
        before.as_ref(),
    )?;

    Ok(stored.into_iter().map(MessageResponse::from).collect())
}

pub async fn get_security_code<S: StateWrapper>(state: S, contact_id: String) -> Result<String> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    let db = state.db_lock().await;
    let contact = Contact::load(&db, &UserId(user_id_bytes))?
        .ok_or(KursalError::Storage("Contact not found".to_string()))?;

    let local_identity_pub = get_local_identity_pub(&db)?;
    let local_dilithium_pub = get_dilithium_pub(&db)?;

    let security = security_code(
        &local_identity_pub,
        &local_dilithium_pub,
        &contact.identity_pub_key,
        &contact.dilithium_pub_key,
    );

    Ok(security)
}

pub async fn confirm_security_code<S: StateWrapper>(state: S, contact_id: String) -> Result<()> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    Contact::set_verified(&*state.db_lock().await, &UserId(user_id_bytes))?;

    Ok(())
}

pub async fn set_contact_blocked<S: StateWrapper>(
    state: S,
    contact_id: String,
    value: bool,
) -> Result<()> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    Contact::set_blocked(&*state.db_lock().await, &UserId(user_id_bytes), value)?;

    Ok(())
}

pub async fn get_blocked_contacts<S: StateWrapper>(state: S) -> Result<Vec<ContactResponse>> {
    let contacts = Contact::load_all(&*state.db_lock().await)?
        .into_iter()
        .filter(|user| user.blocked)
        .map(ContactResponse::from)
        .collect();

    Ok(contacts)
}

pub async fn rotate_peer_id<S: StateWrapper>(state: S) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::RotatePeerId { reply: reply_tx })
        .await
        .ok();

    reply_rx
        .await
        .map_err(|_| KursalError::Network("channel dropped".to_string()))?
}

pub async fn get_local_peer_id<S: StateWrapper>(state: S) -> Result<String> {
    let network = state.network_lock().await;
    Ok(network.primary.peer_id.to_base58())
}

pub async fn get_local_user_id_hex<S: StateWrapper>(state: S) -> Result<String> {
    let db = state.db_lock().await;
    let uid = get_local_user_id(&db)?;
    Ok(hex::encode(uid.0))
}

pub async fn get_local_user_profile<S: StateWrapper>(state: S) -> (String, Option<Vec<u8>>) {
    let db = state.db_lock().await;

    get_local_profile(&db)
}

pub async fn broadcast_profile<S: StateWrapper>(
    state: S,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    set_local_profile(
        &*state.db_lock().await,
        display_name.clone(),
        avatar_bytes.clone(),
    )?;

    state
        .core_cmd_tx()
        .send(CoreCommand::BroadcastProfile {
            display_name,
            avatar_bytes,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?
}

pub async fn share_profile<S: StateWrapper>(
    state: S,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
    contact_id: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::ShareProfile {
            avatar_bytes,
            contact_id,
            display_name,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn delete_message_for_everyone<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::DeleteMessage {
            contact_id,
            message_id,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn edit_message<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
    new_content: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::EditMessage {
            contact_id,
            message_id,
            new_content,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn add_reaction<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
    emoji: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::ReactionAdd {
            contact_id,
            message_id,
            emoji,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn remove_reaction<S: StateWrapper>(
    state: S,
    contact_id: String,
    message_id: String,
    emoji: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::ReactionRemove {
            contact_id,
            message_id,
            emoji,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

pub async fn send_file_offer<S: StateWrapper>(
    state: S,
    contact_id: String,
    file_path: String,
) -> Result<(String, u64)> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::SendFileOffer {
            contact_id,
            file_path,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    let (msg_id, file_size) = reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok((hex::encode(msg_id.0), file_size))
}

pub async fn accept_file_offer<S: StateWrapper>(
    state: S,
    contact_id: String,
    offer_id: String,
    save_path: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx()
        .send(CoreCommand::AcceptFileOffer {
            contact_id,
            offer_id,
            save_path,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}
