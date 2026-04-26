use crate::deep_link::deep_link_handler;
use crate::dirs::{app_data_dir, cache_dir, logs_dir};
use crate::error::Result;
use kursal_core::KursalError;
use kursal_core::api::cmd_wrapper::StateWrapper;
use kursal_core::api::state::AppState;
use kursal_core::api::{CoreCommand, cmd_wrapper};
use kursal_core::apiserver::LocalApiConfig;
use kursal_core::dto::{ContactResponse, MessageResponse, NearbyPeerResponse, OtpResponse};
use kursal_core::network::NetworkManager;
use kursal_core::storage::backup::{generate_backup, load_backup};
use kursal_core::storage::{
    AutoAcceptConfig, AutoDownloadConfig, Database, RelayConfig, SharedFileEntry, StorageUsage,
    api_server_config, delete_message_history_all, delete_message_history_for, files_list_shared,
    files_revoke_shared, get_local_profile, get_local_user_id, get_swarm_listening_port,
    get_swarm_mdns_enabled, hash_new_api_server_password, reset_full_app, set_api_server_config,
    set_api_server_password_hash, set_swarm_listening_port, set_swarm_mdns_enabled,
};
use std::collections::HashMap;
use tauri_plugin_opener::OpenerExt;
use tokio::fs::remove_dir_all;
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
    let file_dir = cache_dir()?.join(&contact_id);

    cmd_wrapper::remove_contact(AppStateWrapper(state), contact_id).await?;
    remove_dir_all(file_dir).await.map_err(KursalError::Io)?;

    Ok(())
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
pub async fn list_blocked_contacts(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ContactResponse>> {
    cmd_wrapper::get_blocked_contacts(AppStateWrapper(state))
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

// OUTSIDE cmd_wrapper / SETTINGS

#[tauri::command]
pub async fn get_storage_usage(state: tauri::State<'_, AppState>) -> Result<StorageUsage> {
    let logs_dir = logs_dir()?;
    let cache_dir = cache_dir()?;

    kursal_core::storage::get_storage_usage(
        &*state.db.0.lock().await,
        logs_dir.to_path_buf(),
        cache_dir.to_path_buf(),
        state.db_path.clone(),
    )
    .map_err(Into::into)
}

#[tauri::command]
pub async fn get_auto_download_config(
    state: tauri::State<'_, AppState>,
) -> Result<AutoDownloadConfig> {
    kursal_core::storage::get_auto_download_config(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn set_auto_download_config(
    state: tauri::State<'_, AppState>,
    config: AutoDownloadConfig,
) -> Result<()> {
    kursal_core::storage::set_auto_download_config(&*state.db.0.lock().await, config)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_auto_accept_config(state: tauri::State<'_, AppState>) -> Result<AutoAcceptConfig> {
    kursal_core::storage::get_auto_accept_config(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn set_auto_accept_config(
    state: tauri::State<'_, AppState>,
    config: AutoAcceptConfig,
) -> Result<()> {
    kursal_core::storage::set_auto_accept_config(&*state.db.0.lock().await, config)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn list_shared_files(state: tauri::State<'_, AppState>) -> Result<Vec<SharedFileEntry>> {
    files_list_shared(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn revoke_shared_file(state: tauri::State<'_, AppState>, id: String) -> Result<()> {
    files_revoke_shared(&*state.db.0.lock().await, id).map_err(Into::into)
}
#[tauri::command]
pub async fn revoke_shared_files_bulk(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<()> {
    let db = state.db.0.lock().await;

    for id in ids {
        files_revoke_shared(&db, id)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_nearby_share_enabled(state: tauri::State<'_, AppState>) -> Result<bool> {
    get_swarm_mdns_enabled(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn set_nearby_share_enabled(
    state: tauri::State<'_, AppState>,
    value: bool,
) -> Result<()> {
    set_swarm_mdns_enabled(&*state.db.0.lock().await, value).map_err(Into::into)
}

#[tauri::command]
pub async fn get_relay_config(state: tauri::State<'_, AppState>) -> Result<RelayConfig> {
    kursal_core::storage::get_relay_config(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn set_relay_config(
    state: tauri::State<'_, AppState>,
    config: RelayConfig,
) -> Result<()> {
    kursal_core::storage::set_relay_config(&*state.db.0.lock().await, config).map_err(Into::into)
}

#[tauri::command]
pub async fn get_listening_port(state: tauri::State<'_, AppState>) -> Result<Option<u16>> {
    Ok(get_swarm_listening_port(&*state.db.0.lock().await))
}
#[tauri::command]
pub async fn set_listening_port(
    state: tauri::State<'_, AppState>,
    port: Option<u16>,
) -> Result<()> {
    set_swarm_listening_port(&*state.db.0.lock().await, port)?;

    Ok(())
}

#[tauri::command]
pub async fn get_local_api_config(state: tauri::State<'_, AppState>) -> Result<LocalApiConfig> {
    api_server_config(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn set_local_api_config(
    state: tauri::State<'_, AppState>,
    config: LocalApiConfig,
) -> Result<()> {
    set_api_server_config(&*state.db.0.lock().await, config).map_err(Into::into)
}
#[tauri::command]
pub async fn generate_local_api_token(state: tauri::State<'_, AppState>) -> Result<String> {
    let (token, hash) = tokio::task::spawn_blocking(hash_new_api_server_password)
        .await
        .map_err(|err| KursalError::Crypto(err.to_string()))??;
    set_api_server_password_hash(&*state.db.0.lock().await, &hash)?;
    Ok(token)
}

#[tauri::command]
pub async fn delete_all_local_data(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<()> {
    reset_full_app(&*state.db.0.lock().await)?;

    app.exit(0);

    Ok(())
}

#[tauri::command]
pub async fn clear_message_history(
    state: tauri::State<'_, AppState>,
    contact_id: Option<String>, // None = ALL CONTACTS
) -> Result<()> {
    if let Some(contact_id) = contact_id {
        delete_message_history_for(&*state.db.0.lock().await, contact_id)?;
    } else {
        delete_message_history_all(&*state.db.0.lock().await)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_peer_rotation_interval(state: tauri::State<'_, AppState>) -> Result<String> {
    let result = match kursal_core::storage::get_peer_rotation_interval(&*state.db.0.lock().await) {
        21_600 => "6h",
        43_200 => "12h",
        108_000 => "30h",
        604_800 => "7d",
        _ => "manual",
    };

    Ok(result.to_string())
}
#[tauri::command]
pub async fn set_peer_rotation_interval(
    state: tauri::State<'_, AppState>,
    interval: String,
) -> Result<()> {
    let result: u64 = match interval.as_str() {
        "6h" => 21_600,
        "12h" => 43_200,
        "30h" => 108_000,
        "7d" => 604_800,
        _ => 0,
    };

    kursal_core::storage::set_peer_rotation_interval(&*state.db.0.lock().await, result)?;

    Ok(())
}

#[tauri::command]
pub async fn get_typing_indicators_enabled(state: tauri::State<'_, AppState>) -> Result<bool> {
    kursal_core::storage::get_typing_indicators_enabled(&*state.db.0.lock().await)
        .map_err(Into::into)
}
#[tauri::command]
pub async fn set_typing_indicators_enabled(
    state: tauri::State<'_, AppState>,
    value: bool,
) -> Result<()> {
    kursal_core::storage::set_typing_indicators_enabled(&*state.db.0.lock().await, value)
        .map_err(Into::into)
}

// OUTSIDE cmd_wrapper

#[tauri::command]
pub async fn export_backup(state: tauri::State<'_, AppState>, password: String) -> Result<Vec<u8>> {
    let app_data_dir = app_data_dir()?;

    generate_backup(
        password,
        &state.keychain_config,
        app_data_dir,
        &state.db_path,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn import_backup(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    password: String,
    bytes: Vec<u8>,
) -> Result<()> {
    let app_data_dir = app_data_dir()?;

    load_backup(
        password,
        bytes,
        &state.db_path,
        &state.keychain_config,
        app_data_dir,
    )
    .await?;

    app.restart()
}

#[tauri::command]
pub async fn get_updater_enabled(state: tauri::State<'_, AppState>) -> Result<bool> {
    kursal_core::storage::get_updater_enabled(&*state.db.0.lock().await).map_err(Into::into)
}
#[tauri::command]
pub async fn set_updater_enabled(state: tauri::State<'_, AppState>, value: bool) -> Result<()> {
    kursal_core::storage::set_updater_enabled(&*state.db.0.lock().await, value).map_err(Into::into)
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

#[tauri::command]
pub async fn open_files_folder(app: tauri::AppHandle) -> Result<()> {
    if let Ok(cache_dir) = crate::dirs::cache_dir() {
        app.opener()
            .open_path(cache_dir.join("files").to_string_lossy(), None::<&str>)
            .map_err(|err| KursalError::Storage(err.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn frontend_ready(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<()> {
    let urls = state.deep_links.lock().await.take();

    if let Some(urls) = urls
        && let Err(err) = deep_link_handler(&app, urls)
    {
        log::error!("Error while handling deep linking: {err}");
    }

    Ok(())
}
