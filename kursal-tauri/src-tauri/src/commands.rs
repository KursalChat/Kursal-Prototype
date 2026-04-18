use crate::error::Result;
use kursal_core::KursalError;
use kursal_core::api::cmd_wrapper::StateWrapper;
use kursal_core::api::state::AppState;
use kursal_core::api::{CoreCommand, cmd_wrapper};
use kursal_core::dto::{ContactResponse, MessageResponse, NearbyPeerResponse, OtpResponse};
use kursal_core::network::NetworkManager;
use kursal_core::storage::{Database, get_local_profile, get_local_user_id};
use std::collections::HashMap;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::{MutexGuard, mpsc, oneshot};

pub struct AppStateWrapper<'a>(pub tauri::State<'a, AppState>);

impl StateWrapper for AppStateWrapper<'_> {
    fn core_cmd_tx(&self) -> &mpsc::Sender<CoreCommand> {
        &self.0.core_cmd_tx
    }
    async fn network_lock(&self) -> MutexGuard<'_, NetworkManager> {
        self.0.network.lock().await
    }
    async fn pending_nearby_lock(&self) -> MutexGuard<'_, HashMap<String, oneshot::Sender<bool>>> {
        self.0.pending_nearby.lock().await
    }
    async fn db_lock(&self) -> MutexGuard<'_, Database> {
        self.0.db.0.lock().await
    }
}

#[tauri::command]
pub async fn generate_otp() -> Result<OtpResponse> {
    cmd_wrapper::generate_otp().await.map_err(Into::into)
}

#[tauri::command]
pub async fn publish_otp(state: tauri::State<'_, AppState>, otp: String) -> Result<()> {
    cmd_wrapper::publish_otp(AppStateWrapper(state), otp)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn fetch_otp(state: tauri::State<'_, AppState>, otp: String) -> Result<ContactResponse> {
    cmd_wrapper::fetch_otp(AppStateWrapper(state), otp)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn export_ltc(state: tauri::State<'_, AppState>) -> Result<Vec<u8>> {
    cmd_wrapper::export_ltc(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn import_ltc(
    state: tauri::State<'_, AppState>,
    bytes: Vec<u8>,
) -> Result<ContactResponse> {
    cmd_wrapper::import_ltc(AppStateWrapper(state), bytes)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn start_nearby(state: tauri::State<'_, AppState>) -> Result<String> {
    cmd_wrapper::start_nearby(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn stop_nearby(state: tauri::State<'_, AppState>) -> Result<()> {
    cmd_wrapper::stop_nearby(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_nearby_peers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<NearbyPeerResponse>> {
    cmd_wrapper::get_nearby_peers(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn connect_nearby(
    state: tauri::State<'_, AppState>,
    peer_id: String,
    method: String,
) -> Result<()> {
    cmd_wrapper::connect_nearby(AppStateWrapper(state), peer_id, method)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn accept_nearby(state: tauri::State<'_, AppState>, peer_id: String) -> Result<()> {
    cmd_wrapper::accept_nearby(AppStateWrapper(state), peer_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn decline_nearby(state: tauri::State<'_, AppState>, peer_id: String) -> Result<()> {
    cmd_wrapper::decline_nearby(AppStateWrapper(state), peer_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_contacts(state: tauri::State<'_, AppState>) -> Result<Vec<ContactResponse>> {
    cmd_wrapper::get_contacts(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn remove_contact(state: tauri::State<'_, AppState>, contact_id: String) -> Result<()> {
    cmd_wrapper::remove_contact(AppStateWrapper(state), contact_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn send_text(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    text: String,
    reply_to: Option<String>,
) -> Result<String> {
    cmd_wrapper::send_text(AppStateWrapper(state), contact_id, text, reply_to)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn send_typing_indicator(
    state: tauri::State<'_, AppState>,
    contact_id: String,
) -> Result<()> {
    cmd_wrapper::send_typing_indicator(AppStateWrapper(state), contact_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn delete_local_message(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    message_id: String,
) -> Result<()> {
    cmd_wrapper::delete_local_message(AppStateWrapper(state), contact_id, message_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_messages(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    limit: usize,
    before: Option<String>,
) -> Result<Vec<MessageResponse>> {
    cmd_wrapper::get_messages(AppStateWrapper(state), contact_id, limit, before)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_security_code(
    state: tauri::State<'_, AppState>,
    contact_id: String,
) -> Result<String> {
    cmd_wrapper::get_security_code(AppStateWrapper(state), contact_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn confirm_security_code(
    state: tauri::State<'_, AppState>,
    contact_id: String,
) -> Result<()> {
    cmd_wrapper::confirm_security_code(AppStateWrapper(state), contact_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn set_contact_blocked(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    value: bool,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(AppStateWrapper(state), contact_id, value)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn rotate_peer_id(state: tauri::State<'_, AppState>) -> Result<()> {
    cmd_wrapper::rotate_peer_id(AppStateWrapper(state))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_local_peer_id(state: tauri::State<'_, AppState>) -> Result<String> {
    cmd_wrapper::get_local_peer_id(AppStateWrapper(state))
        .await
        .map_err(Into::into)
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
pub async fn get_local_user_id_hex(state: tauri::State<'_, AppState>) -> Result<String> {
    let db = state.db.0.lock().await;
    let uid = get_local_user_id(&db)?;
    Ok(hex::encode(uid.0))
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
    cmd_wrapper::broadcast_profile(AppStateWrapper(state), display_name, avatar_bytes)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn share_profile(
    state: tauri::State<'_, AppState>,
    display_name: String,
    avatar_bytes: Option<Vec<u8>>,
    contact_id: String,
) -> Result<()> {
    cmd_wrapper::share_profile(
        AppStateWrapper(state),
        display_name,
        avatar_bytes,
        contact_id,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn delete_message_for_everyone(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    message_id: String,
) -> Result<()> {
    cmd_wrapper::delete_message_for_everyone(AppStateWrapper(state), contact_id, message_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn edit_message(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    message_id: String,
    new_content: String,
) -> Result<()> {
    cmd_wrapper::edit_message(AppStateWrapper(state), contact_id, message_id, new_content)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn add_reaction(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    message_id: String,
    emoji: String,
) -> Result<()> {
    cmd_wrapper::add_reaction(AppStateWrapper(state), contact_id, message_id, emoji)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn remove_reaction(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    message_id: String,
    emoji: String,
) -> Result<()> {
    cmd_wrapper::remove_reaction(AppStateWrapper(state), contact_id, message_id, emoji)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn send_file_offer(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    file_path: String,
) -> Result<(String, u64)> {
    cmd_wrapper::send_file_offer(AppStateWrapper(state), contact_id, file_path)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn accept_file_offer(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    offer_id: String,
    save_path: String,
) -> Result<()> {
    cmd_wrapper::accept_file_offer(AppStateWrapper(state), contact_id, offer_id, save_path)
        .await
        .map_err(Into::into)
}

// OUTSIDE cmd_wrapper

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
