use crate::dto::{ContactResponse, MessageResponse, NearbyPeerResponse, OtpResponse};
use crate::error::Result;
use kursal_core::KursalError;
use kursal_core::api::CoreCommand;
use kursal_core::api::state::AppState;
use kursal_core::contacts::Contact;
use kursal_core::first_contact::nearby::{NearbyBeacon, generate_session_name};
use kursal_core::first_contact::otp;
use kursal_core::identity::{UserId, security_code};
use kursal_core::messaging::StoredMessage;
use kursal_core::messaging::enums::MessageId;
use kursal_core::storage::{
    get_dilithium_pub, get_local_identity_pub, get_local_profile, set_local_profile,
};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::oneshot;

#[tauri::command]
pub async fn generate_otp() -> Result<OtpResponse> {
    Ok(OtpResponse {
        otp: otp::generate_otp()?,
    })
}

#[tauri::command]
pub async fn publish_otp(state: tauri::State<'_, AppState>, otp: String) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
        .send(CoreCommand::PublishOtp {
            otp,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn fetch_otp(state: tauri::State<'_, AppState>, otp: String) -> Result<ContactResponse> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
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

#[tauri::command]
pub async fn export_ltc(state: tauri::State<'_, AppState>) -> Result<Vec<u8>> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
        .send(CoreCommand::ExportLtc { reply: reply_tx })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn import_ltc(
    state: tauri::State<'_, AppState>,
    bytes: Vec<u8>,
) -> Result<ContactResponse> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
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

#[tauri::command]
pub async fn start_nearby(state: tauri::State<'_, AppState>) -> Result<String> {
    let session_name = generate_session_name()?;
    let mut network = state.network.lock().await;

    let beacon = NearbyBeacon {
        peer_id: network.primary.peer_id.to_base58(),
        session_name: session_name.clone(),
    };

    network.start_mdns(beacon).await?;

    Ok(session_name)
}

#[tauri::command]
pub async fn stop_nearby(state: tauri::State<'_, AppState>) -> Result<()> {
    let mut network = state.network.lock().await;

    network.stop_mdns().await?;

    Ok(())
}

#[tauri::command]
pub async fn get_nearby_peers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<NearbyPeerResponse>> {
    let network = state.network.lock().await;

    let peers = kursal_core::network::get_nearby_peers(&network).await;

    Ok(peers
        .into_iter()
        .map(|(_, beacon)| NearbyPeerResponse::from(beacon))
        .collect())
}

#[tauri::command]
pub async fn connect_nearby(state: tauri::State<'_, AppState>, peer_id: String) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    let session_name = {
        let network = state.network.lock().await;

        let transport = network
            .mdns_transport
            .clone()
            .ok_or(KursalError::Network("Nearby share not active".to_string()))?;

        transport
            .my_beacon
            .lock()
            .await
            .as_ref()
            .map(|b| b.session_name.clone())
            .ok_or(KursalError::Network("No active beacon".to_string()))?
    };

    state
        .core_cmd_tx
        .send(CoreCommand::ConnectNearby {
            peer_id,
            session_name,
            reply: reply_tx,
        })
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    reply_rx
        .await
        .map_err(|err| KursalError::Network(err.to_string()))??;

    Ok(())
}

#[tauri::command]
pub async fn accept_nearby(state: tauri::State<'_, AppState>, peer_id: String) -> Result<()> {
    if let Some(tx) = state.pending_nearby.lock().await.remove(&peer_id) {
        tx.send(true).ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn decline_nearby(state: tauri::State<'_, AppState>, peer_id: String) -> Result<()> {
    if let Some(tx) = state.pending_nearby.lock().await.remove(&peer_id) {
        tx.send(false).ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn get_contacts(state: tauri::State<'_, AppState>) -> Result<Vec<ContactResponse>> {
    let contacts = Contact::load_all(&*state.db.clone().0.lock().await)?;

    Ok(contacts.into_iter().map(ContactResponse::from).collect())
}

#[tauri::command]
pub async fn remove_contact(state: tauri::State<'_, AppState>, contact_id: String) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
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

#[tauri::command]
pub async fn send_text(
    state: tauri::State<'_, AppState>,
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
        .core_cmd_tx
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

#[tauri::command]
pub async fn delete_local_message(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    message_id: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
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

#[tauri::command]
pub async fn get_messages(
    state: tauri::State<'_, AppState>,
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
        &*state.db.0.lock().await,
        &UserId(user_id_bytes),
        limit,
        before.as_ref(),
    )?;

    Ok(stored.into_iter().map(MessageResponse::from).collect())
}

#[tauri::command]
pub async fn get_security_code(
    state: tauri::State<'_, AppState>,
    contact_id: String,
) -> Result<String> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    let db = state.db.0.lock().await;
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

#[tauri::command]
pub async fn confirm_security_code(
    state: tauri::State<'_, AppState>,
    contact_id: String,
) -> Result<()> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    Contact::set_verified(&*state.db.0.lock().await, &UserId(user_id_bytes))?;

    Ok(())
}

#[tauri::command]
pub async fn set_contact_blocked(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    value: bool,
) -> Result<()> {
    let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
        .map_err(|err| KursalError::Crypto(err.to_string()))?
        .try_into()
        .map_err(|_| KursalError::Crypto("Invalid contact id length".to_string()))?;

    Contact::set_blocked(&*state.db.0.lock().await, &UserId(user_id_bytes), value)?;

    Ok(())
}

#[tauri::command]
pub async fn rotate_peer_id(state: tauri::State<'_, AppState>) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
        .send(CoreCommand::RotatePeerId { reply: reply_tx })
        .await
        .ok();

    Ok(reply_rx
        .await
        .map_err(|_| KursalError::Network("channel dropped".to_string()))??)
}

#[tauri::command]
pub async fn get_local_peer_id(state: tauri::State<'_, AppState>) -> Result<String> {
    let network = state.network.lock().await;
    Ok(network.primary.peer_id.to_base58())
}

#[tauri::command]
pub async fn set_relay_server_enabled(
    state: tauri::State<'_, AppState>,
    value: bool,
) -> Result<()> {
    kursal_core::storage::set_relay_server_enabled(&*state.db.clone().0.lock().await, value)?;

    Ok(())
}

#[tauri::command]
pub async fn get_local_user_profile(
    state: tauri::State<'_, AppState>,
) -> Result<(String, Option<Vec<u8>>)> {
    let db = state.db.0.lock().await;

    get_local_profile(&db).map_err(Into::into)
}

#[tauri::command]
pub async fn broadcast_profile(
    state: tauri::State<'_, AppState>,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    set_local_profile(
        &*state.db.0.lock().await,
        display_name.clone(),
        avatar_bytes.clone(),
    )?;

    state
        .core_cmd_tx
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
        .map_err(Into::into)
}

#[tauri::command]
pub async fn share_profile(
    state: tauri::State<'_, AppState>,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
    contact_id: String,
) -> Result<()> {
    let (reply_tx, reply_rx) = oneshot::channel();

    state
        .core_cmd_tx
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

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> std::result::Result<(), String> {
    crate::check_for_updates_impl(app, true)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[tauri::command]
pub async fn check_for_updates(_app: tauri::AppHandle) -> std::result::Result<(), String> {
    Err("Updates are handled by the app store on mobile devices.".to_string())
}

#[tauri::command]
pub async fn open_log_folder(app: tauri::AppHandle) -> Result<()> {
    if let Ok(log_dir) = crate::dirs::logs_dir() {
        app.opener()
            .open_path(log_dir.to_string_lossy(), None::<&str>)
            .map_err(|err| KursalError::Storage(err.to_string()))?;
    }
    Ok(())
}
