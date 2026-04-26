use crate::{
    KursalError,
    api::{
        CoreCommand,
        cmd_wrapper::{self, StateWrapper},
    },
    apiserver::iprecord::IpRecord,
    dto::{ContactResponse, MessageResponse, NearbyPeerResponse, OtpResponse},
    network::NetworkManager,
    storage::{Database, SharedDatabase},
};
use auth::auth_middleware;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    middleware,
    response::IntoResponse,
    routing::{any, delete, get, patch, post},
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::sync::{Mutex, MutexGuard, broadcast, mpsc, oneshot};
use tower_http::cors::CorsLayer;

pub mod auth;
pub mod iprecord;

type Result<T> = std::result::Result<T, String>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiConfig {
    pub enabled: bool,
    pub host_on_network: bool,
    pub port: u16,
}
impl LocalApiConfig {
    pub fn serialize(&self) -> crate::Result<Vec<u8>> {
        bincode::serialize(self).map_err(|err| KursalError::Storage(err.to_string()))
    }
    pub fn deserialize(bytes: &[u8]) -> crate::Result<Self> {
        bincode::deserialize(bytes).map_err(|err| KursalError::Storage(err.to_string()))
    }
}

#[derive(Clone)]
pub struct CoreEventEmitter {
    pub event: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct APIAppState {
    auth_token: String,
    rate_map: Arc<Mutex<HashMap<IpAddr, IpRecord>>>,
    //
    db: SharedDatabase,
    core_cmd_tx: mpsc::Sender<CoreCommand>,
    network: Arc<Mutex<NetworkManager>>,
    pending_nearby: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    event_tx: broadcast::Sender<CoreEventEmitter>,
}

impl StateWrapper for APIAppState {
    fn core_cmd_tx(&self) -> &mpsc::Sender<CoreCommand> {
        &self.core_cmd_tx
    }
    async fn network_lock(&self) -> MutexGuard<'_, NetworkManager> {
        self.network.lock().await
    }
    async fn pending_nearby_lock(&self) -> MutexGuard<'_, HashMap<String, oneshot::Sender<bool>>> {
        self.pending_nearby.lock().await
    }
    async fn db_lock(&self) -> MutexGuard<'_, Database> {
        self.db.0.lock().await
    }
}

pub async fn run_server(
    auth_token: String,
    api_config: LocalApiConfig,
    core_cmd_tx: mpsc::Sender<CoreCommand>,
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    pending_nearby: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    event_tx: broadcast::Sender<CoreEventEmitter>,
) -> crate::Result<()> {
    let state = APIAppState {
        auth_token,
        rate_map: Arc::default(),
        core_cmd_tx,
        db,
        network,
        pending_nearby,
        event_tx,
    };

    let app = Router::new()
        .layer(CorsLayer::new())
        .route("/", get(root))
        .route("/ws", any(ws_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        //
        .route("/self/user_id", get(api_self_get_user_id))
        .route("/self/profile", get(api_self_get_profile))
        .route("/self/profile", post(api_self_post_profile))
        .route("/self/peer_id", get(api_self_get_peer_id))
        .route("/self/peer_id", delete(api_self_rotate_peer_id))
        //
        .route("/otp/generate", post(api_otp_generate))
        .route("/otp/publish", post(api_otp_publish))
        .route("/otp/fetch", post(api_otp_fetch))
        //
        .route("/ltc/export", post(api_ltc_export))
        .route("/ltc/import", post(api_ltc_import))
        //
        .route("/nearby/start", post(api_nearby_start))
        .route("/nearby/stop", post(api_nearby_stop))
        .route("/nearby", get(api_nearby_get))
        .route(
            "/nearby/{peer_id}/connect/{method}",
            get(api_nearby_connect),
        )
        .route("/nearby/{peer_id}/accept", get(api_nearby_accept))
        .route("/nearby/{peer_id}/decline", get(api_nearby_decline))
        //
        .route("/contacts", get(api_contacts))
        .route(
            "/contact/{user_id}/security_code",
            get(api_contact_security_code),
        )
        .route(
            "/contact/{user_id}/security_code",
            post(api_contact_security_code_confirm),
        )
        .route(
            "/contact/{user_id}/profile",
            post(api_contact_share_profile),
        )
        .route("/contact/{user_id}", delete(api_contact_remove))
        .route("/contacts/blocked", get(api_contact_blocked_list))
        .route("/contact/{user_id}/block", post(api_contact_block))
        .route("/contact/{user_id}/unblock", post(api_contact_unblock))
        //
        .route("/typing", post(api_typing))
        .route("/messages/{contact_id}", post(api_messages_send))
        .route("/messages/{contact_id}", get(api_messages_get))
        .route(
            "/messages/{contact_id}/{message_id}/local",
            delete(api_message_delete_local),
        )
        .route(
            "/messages/{contact_id}/{message_id}",
            delete(api_message_delete),
        )
        .route(
            "/messages/{contact_id}/{message_id}",
            patch(api_message_edit),
        )
        .route(
            "/messages/{contact_id}/{message_id}/reactions/{emoji}",
            post(api_message_reaction_add),
        )
        .route(
            "/messages/{contact_id}/{message_id}/reactions/{emoji}",
            delete(api_message_reaction_remove),
        )
        //
        .route("/files/{contact_id}", post(api_files_send))
        .route("/files/{contact_id}/{offer_id}", post(api_files_accept))
        //
        .with_state(state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let listener = tokio::net::TcpListener::bind(format!(
        "{}:{}",
        if api_config.host_on_network {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        },
        api_config.port
    )) // "127.0.0.1:4892"
    .await
    .map_err(|err| KursalError::Network(err.to_string()))?;

    axum::serve(listener, app)
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    Ok(())
}

async fn root() -> String {
    format!("Kursal v{}\n", env!("CARGO_PKG_VERSION"))
}

//

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<APIAppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: APIAppState) {
    let mut event_rx = state.event_tx.subscribe();
    let (mut sink, mut stream) = socket.split();

    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let msg = serde_json::json!({
                        "event": event.event,
                        "payload": event.payload,
                    });

                    if sink
                        .send(Message::Text(msg.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Websocket client lagged, dropped {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }

    send_task.abort();
}

//

async fn api_otp_generate() -> Result<Json<OtpResponse>> {
    cmd_wrapper::generate_otp()
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_otp_publish(
    State(state): State<APIAppState>,
    Json(OtpResponse { otp }): Json<OtpResponse>,
) -> Result<()> {
    cmd_wrapper::publish_otp(state, otp)
        .await
        .map_err(Into::into)
}

async fn api_otp_fetch(
    State(state): State<APIAppState>,
    Json(OtpResponse { otp }): Json<OtpResponse>,
) -> Result<Json<ContactResponse>> {
    cmd_wrapper::fetch_otp(state, otp)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_ltc_export(State(state): State<APIAppState>) -> Result<Vec<u8>> {
    cmd_wrapper::export_ltc(state).await.map_err(Into::into)
}

async fn api_ltc_import(
    State(state): State<APIAppState>,
    bytes: Bytes,
) -> Result<Json<ContactResponse>> {
    cmd_wrapper::import_ltc(state, bytes.to_vec())
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_nearby_start(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::start_nearby(state).await.map_err(Into::into)
}

async fn api_nearby_stop(State(state): State<APIAppState>) -> Result<()> {
    cmd_wrapper::stop_nearby(state).await.map_err(Into::into)
}

async fn api_nearby_get(State(state): State<APIAppState>) -> Result<Json<Vec<NearbyPeerResponse>>> {
    cmd_wrapper::get_nearby_peers(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_nearby_connect(
    State(state): State<APIAppState>,
    Path((peer_id, method)): Path<(String, String)>,
) -> Result<()> {
    if method != "mdns" && method != "bluetooth" {
        return Err("Method can only be mdns or bluetooth".to_string());
    }

    cmd_wrapper::connect_nearby(state, peer_id, method)
        .await
        .map_err(Into::into)
}

async fn api_nearby_accept(
    State(state): State<APIAppState>,
    Path(peer_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::accept_nearby(state, peer_id)
        .await
        .map_err(Into::into)
}

async fn api_nearby_decline(
    State(state): State<APIAppState>,
    Path(peer_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::decline_nearby(state, peer_id)
        .await
        .map_err(Into::into)
}

async fn api_contacts(State(state): State<APIAppState>) -> Result<Json<Vec<ContactResponse>>> {
    cmd_wrapper::get_contacts(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_contact_remove(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    // TODO: remove file transfers
    cmd_wrapper::remove_contact(state, contact_id)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize)]
struct APIMessageSend {
    text: String,
    reply_to: Option<String>,
}
async fn api_messages_send(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APIMessageSend { text, reply_to }): Json<APIMessageSend>,
) -> Result<String> {
    cmd_wrapper::send_text(state, contact_id, text, reply_to)
        .await
        .map_err(Into::into)
}

async fn api_typing(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::send_typing_indicator(state, contact_id)
        .await
        .map_err(Into::into)
}

async fn api_message_delete_local(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::delete_local_message(state, contact_id, message_id)
        .await
        .map_err(Into::into)
}

async fn api_message_delete(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::delete_message_for_everyone(state, contact_id, message_id)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize)]
struct APIMessageEdit {
    text: String,
}
async fn api_message_edit(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
    Json(APIMessageEdit { text }): Json<APIMessageEdit>,
) -> Result<()> {
    cmd_wrapper::edit_message(state, contact_id, message_id, text)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize)]
struct APIMessagesGet {
    limit: usize,
    before: Option<String>,
}
async fn api_messages_get(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    query: Query<APIMessagesGet>,
) -> Result<Json<Vec<MessageResponse>>> {
    let query: APIMessagesGet = query.0;

    cmd_wrapper::get_messages(state, contact_id, query.limit, query.before)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_message_reaction_add(
    State(state): State<APIAppState>,
    Path((contact_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<()> {
    cmd_wrapper::add_reaction(state, contact_id, message_id, emoji)
        .await
        .map_err(Into::into)
}

async fn api_message_reaction_remove(
    State(state): State<APIAppState>,
    Path((contact_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<()> {
    cmd_wrapper::remove_reaction(state, contact_id, message_id, emoji)
        .await
        .map_err(Into::into)
}

async fn api_contact_security_code(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<String> {
    cmd_wrapper::get_security_code(state, contact_id)
        .await
        .map_err(Into::into)
}

async fn api_contact_security_code_confirm(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::confirm_security_code(state, contact_id)
        .await
        .map_err(Into::into)
}

async fn api_contact_block(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(state, contact_id, true)
        .await
        .map_err(Into::into)
}

async fn api_contact_unblock(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(state, contact_id, false)
        .await
        .map_err(Into::into)
}

async fn api_contact_blocked_list(
    State(state): State<APIAppState>,
) -> Result<Json<Vec<ContactResponse>>> {
    cmd_wrapper::get_blocked_contacts(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn api_self_rotate_peer_id(State(state): State<APIAppState>) -> Result<()> {
    cmd_wrapper::rotate_peer_id(state).await.map_err(Into::into)
}

async fn api_self_get_peer_id(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::get_local_peer_id(state)
        .await
        .map_err(Into::into)
}

async fn api_self_get_user_id(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::get_local_user_id_hex(state)
        .await
        .map_err(Into::into)
}

#[derive(Serialize, Deserialize)]
struct APISelfProfile {
    username: String,
    avatar: Option<Vec<u8>>,
}
async fn api_self_get_profile(State(state): State<APIAppState>) -> Result<Json<APISelfProfile>> {
    cmd_wrapper::get_local_user_profile(state)
        .await
        .map(|(username, avatar)| Json(APISelfProfile { username, avatar }))
        .map_err(Into::into)
}

async fn api_self_post_profile(
    State(state): State<APIAppState>,
    Json(APISelfProfile { username, avatar }): Json<APISelfProfile>,
) -> Result<()> {
    cmd_wrapper::broadcast_profile(state, username, avatar)
        .await
        .map_err(Into::into)
}

async fn api_contact_share_profile(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APISelfProfile { username, avatar }): Json<APISelfProfile>,
) -> Result<()> {
    cmd_wrapper::share_profile(state, username, avatar, contact_id)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize)]
struct APIFile {
    path: String,
}
#[derive(Serialize)]
struct APIFileDetails {
    name: String,
    size: u64,
}

async fn api_files_send(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APIFile { path }): Json<APIFile>,
) -> Result<Json<APIFileDetails>> {
    cmd_wrapper::send_file_offer(state, contact_id, path)
        .await
        .map(|(name, size)| Json(APIFileDetails { name, size }))
        .map_err(Into::into)
}

async fn api_files_accept(
    State(state): State<APIAppState>,
    Path((contact_id, offer_id)): Path<(String, String)>,
    Json(APIFile { path }): Json<APIFile>,
) -> Result<()> {
    cmd_wrapper::accept_file_offer(state, contact_id, offer_id, path)
        .await
        .map_err(Into::into)
}
