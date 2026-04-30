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
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};

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

struct BearerAuth;
impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearerAuth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Kursal API",
    ),
    modifiers(&BearerAuth),
    security(("bearerAuth" = [])),
    tags(
        (name = "Self",       description = "Current user identity & profile"),
        (name = "OTP",        description = "One-time password operations"),
        (name = "LTC",        description = "Long-term credential import/export"),
        (name = "Nearby",     description = "Nearby peer discovery & connection"),
        (name = "Contacts",   description = "Contact management & blocking"),
        (name = "Messages",   description = "Manage messages"),
    ),
    paths(
        api_self_get_user_id, api_self_get_profile, api_self_post_profile, api_self_get_peer_id, api_self_rotate_peer_id,
        api_otp_generate, api_otp_fetch,
        api_ltc_export, api_ltc_import,
        api_nearby_start, api_nearby_stop, api_nearby_get, api_nearby_connect, api_nearby_accept, api_nearby_decline,
        api_contacts, api_contact_security_code, api_contact_security_code_confirm, api_contact_share_profile, api_contact_get, api_contact_remove, api_contact_blocked_list, api_contact_block, api_contact_unblock,
        api_typing, api_messages_send, api_messages_get, api_message_delete_local, api_message_delete, api_message_edit, api_message_reaction_add, api_message_reaction_remove, api_files_send, api_files_accept,
    ),
    components(schemas(
        APISelfProfile, OtpResponse, ContactResponse, NearbyPeerResponse, MessageResponse, APIFile, APIFileDetails,
    )),
)]
struct ApiDoc;

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

    let protected = Router::new()
        .route("/ws", any(ws_handler))
        //
        .route("/self/user_id", get(api_self_get_user_id))
        .route("/self/profile", get(api_self_get_profile))
        .route("/self/profile", post(api_self_post_profile))
        .route("/self/peer_id", get(api_self_get_peer_id))
        .route("/self/peer_id", delete(api_self_rotate_peer_id))
        //
        .route("/otp/generate", post(api_otp_generate))
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
            post(api_nearby_connect),
        )
        .route("/nearby/{peer_id}/accept", post(api_nearby_accept))
        .route("/nearby/{peer_id}/decline", post(api_nearby_decline))
        //
        .route("/contacts", get(api_contacts))
        .route(
            "/contact/{contact_id}/security_code",
            get(api_contact_security_code),
        )
        .route(
            "/contact/{contact_id}/security_code",
            post(api_contact_security_code_confirm),
        )
        .route(
            "/contact/{contact_id}/profile",
            post(api_contact_share_profile),
        )
        .route("/contact/{contact_id}", get(api_contact_get))
        .route("/contact/{contact_id}", delete(api_contact_remove))
        .route("/contacts/blocked", get(api_contact_blocked_list))
        .route("/contact/{contact_id}/block", post(api_contact_block))
        .route("/contact/{contact_id}/unblock", post(api_contact_unblock))
        //
        .route("/typing/{contact_id}", post(api_typing))
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
        .route("/files/{contact_id}", post(api_files_send))
        .route("/files/{contact_id}/{offer_id}", post(api_files_accept))
        //
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(protected)
        .merge(utoipa_swagger_ui::SwaggerUi::new("/").url("/openapi.json", ApiDoc::openapi()))
        .layer(CorsLayer::new())
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
    ))
    .await
    .map_err(|err| KursalError::Network(err.to_string()))?;

    axum::serve(listener, app)
        .await
        .map_err(|err| KursalError::Network(err.to_string()))?;

    Ok(())
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

#[utoipa::path(
    post,
    path = "/otp/generate",
    tag = "OTP",
    responses(
        (status = 200, description = "Generated OTP", body = OtpResponse)
    )
)]
async fn api_otp_generate(State(state): State<APIAppState>) -> Result<Json<OtpResponse>> {
    let otp = cmd_wrapper::generate_otp().await?;
    cmd_wrapper::publish_otp(state, otp.otp.clone()).await?;

    Ok(Json(otp))
}

#[utoipa::path(
    post,
    path = "/otp/fetch",
    tag = "OTP",
    request_body = OtpResponse,
    responses(
        (status = 200, description = "Fetched OTP", body = ContactResponse)
    )
)]
async fn api_otp_fetch(
    State(state): State<APIAppState>,
    Json(OtpResponse { otp }): Json<OtpResponse>,
) -> Result<Json<ContactResponse>> {
    cmd_wrapper::fetch_otp(state, otp)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/ltc/export",
    tag = "LTC",
    responses(
        (status = 200, description = "Exported LTC", body = Vec<u8>)
    )
)]
async fn api_ltc_export(State(state): State<APIAppState>) -> Result<Vec<u8>> {
    cmd_wrapper::export_ltc(state).await.map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/ltc/import",
    tag = "LTC",
    request_body(
        content = Vec<u8>,
        content_type = "application/octet-stream"
    ),
    responses(
        (status = 200, description = "Imported LTC", body = ContactResponse)
    )
)]
async fn api_ltc_import(
    State(state): State<APIAppState>,
    bytes: Bytes,
) -> Result<Json<ContactResponse>> {
    cmd_wrapper::import_ltc(state, bytes.to_vec())
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/start",
    tag = "Nearby",
    responses(
        (status = 200, description = "Started Nearby", body = String)
    )
)]
async fn api_nearby_start(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::start_nearby(state).await.map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/stop",
    tag = "Nearby",
    responses(
        (status = 200, description = "Stopped Nearby")
    )
)]
async fn api_nearby_stop(State(state): State<APIAppState>) -> Result<()> {
    cmd_wrapper::stop_nearby(state).await.map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/nearby",
    tag = "Nearby",
    responses(
        (status = 200, description = "Got Nearby peers", body = Vec<NearbyPeerResponse>)
    )
)]
async fn api_nearby_get(State(state): State<APIAppState>) -> Result<Json<Vec<NearbyPeerResponse>>> {
    cmd_wrapper::get_nearby_peers(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/{peer_id}/connect/{method}",
    tag = "Nearby",
    params(
        ("peer_id" = String, Path, description = "Peer ID"),
        ("method" = String, Path, description = "Connection method (bluetooth or mdns)"),
    ),
    responses(
        (status = 200, description = "Connected")
    )
)]
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

#[utoipa::path(
    post,
    path = "/nearby/{peer_id}/accept",
    tag = "Nearby",
    params(
        ("peer_id" = String, Path, description = "Peer ID"),
    ),
    responses(
        (status = 200, description = "Accepted Nearby connection")
    )
)]
async fn api_nearby_accept(
    State(state): State<APIAppState>,
    Path(peer_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::accept_nearby(state, peer_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/{peer_id}/decline",
    tag = "Nearby",
    params(
        ("peer_id" = String, Path, description = "Peer ID"),
    ),
    responses(
        (status = 200, description = "Declined Nearby connection")
    )
)]
async fn api_nearby_decline(
    State(state): State<APIAppState>,
    Path(peer_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::decline_nearby(state, peer_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contacts",
    tag = "Contacts",
    responses(
        (status = 200, description = "Got contacts", body = Vec<ContactResponse>)
    )
)]
async fn api_contacts(State(state): State<APIAppState>) -> Result<Json<Vec<ContactResponse>>> {
    cmd_wrapper::get_contacts(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contact/{contact_id}",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Fetched contact", body = Option<ContactResponse>)
    )
)]
async fn api_contact_get(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<Json<Option<ContactResponse>>> {
    cmd_wrapper::get_contact(state, contact_id)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/contact/{contact_id}",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Removed contact")
    )
)]
async fn api_contact_remove(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    // TODO: remove file transfers
    cmd_wrapper::remove_contact(state, contact_id)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize, ToSchema)]
struct APIMessageSend {
    text: String,
    reply_to: Option<String>,
}

#[utoipa::path(
    post,
    path = "/messages/{contact_id}",
    tag = "Messages",
    request_body = APIMessageSend,
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Sent message", body = String)
    )
)]
async fn api_messages_send(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APIMessageSend { text, reply_to }): Json<APIMessageSend>,
) -> Result<String> {
    cmd_wrapper::send_text(state, contact_id, text, reply_to)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/typing/{contact_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID")
    ),
    responses(
        (status = 200, description = "Typing indicator sent")
    )
)]
async fn api_typing(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::send_typing_indicator(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/messages/{contact_id}/{message_id}/local",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Deleted message locally")
    )
)]
async fn api_message_delete_local(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::delete_local_message(state, contact_id, message_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/messages/{contact_id}/{message_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Deleted message")
    )
)]
async fn api_message_delete(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::delete_message_for_everyone(state, contact_id, message_id)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize, ToSchema)]
struct APIMessageEdit {
    text: String,
}

#[utoipa::path(
    patch,
    path = "/messages/{contact_id}/{message_id}",
    tag = "Messages",
    request_body = APIMessageEdit,
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Edited message")
    )
)]
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

#[utoipa::path(
    get,
    path = "/messages/{contact_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("limit" = usize, Query, description = "Message count"),
        ("before" = Option<String>, Query, description = "Before a certain message"),
    ),
    responses(
        (status = 200, description = "Sent message", body = Vec<MessageResponse>)
    )
)]
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

#[utoipa::path(
    post,
    path = "/messages/{contact_id}/{message_id}/reactions/{emoji}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
        ("emoji" = String, Path, description = "Emoji"),
    ),
    responses(
        (status = 200, description = "Sent reaction")
    )
)]
async fn api_message_reaction_add(
    State(state): State<APIAppState>,
    Path((contact_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<()> {
    cmd_wrapper::add_reaction(state, contact_id, message_id, emoji)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/messages/{contact_id}/{message_id}/reactions/{emoji}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
        ("emoji" = String, Path, description = "Emoji"),
    ),
    responses(
        (status = 200, description = "Removed reaction")
    )
)]
async fn api_message_reaction_remove(
    State(state): State<APIAppState>,
    Path((contact_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<()> {
    cmd_wrapper::remove_reaction(state, contact_id, message_id, emoji)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contact/{contact_id}/security_code",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Got security code")
    )
)]
async fn api_contact_security_code(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<String> {
    cmd_wrapper::get_security_code(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/security_code",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Validated security code")
    )
)]
async fn api_contact_security_code_confirm(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::confirm_security_code(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/block",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Blocked contact")
    )
)]
async fn api_contact_block(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(state, contact_id, true)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/unblock",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Unblocked contact")
    )
)]
async fn api_contact_unblock(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(state, contact_id, false)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contacts/blocked",
    tag = "Contacts",
    responses(
        (status = 200, description = "Listed blocked contacts", body = Vec<ContactResponse>)
    )
)]
async fn api_contact_blocked_list(
    State(state): State<APIAppState>,
) -> Result<Json<Vec<ContactResponse>>> {
    cmd_wrapper::get_blocked_contacts(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/self_peer_id",
    tag = "Self",
    responses(
        (status = 200, description = "Peer ID rotated")
    )
)]
async fn api_self_rotate_peer_id(State(state): State<APIAppState>) -> Result<()> {
    cmd_wrapper::rotate_peer_id(state).await.map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/self/peer_id",
    tag = "Self",
    responses(
        (status = 200, description = "Peer ID", body = String)
    )
)]
async fn api_self_get_peer_id(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::get_local_peer_id(state)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/self/user_id",
    tag = "Self",
    responses(
        (status = 200, description = "User ID", body = String)
    )
)]
async fn api_self_get_user_id(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::get_local_user_id_hex(state)
        .await
        .map_err(Into::into)
}

#[derive(Serialize, Deserialize, ToSchema)]
struct APISelfProfile {
    username: String,
    avatar: Option<Vec<u8>>,
}

#[utoipa::path(
    get,
    path = "/self/profile",
    tag = "Self",
    responses(
        (status = 200, description = "User profile", body = APISelfProfile)
    )
)]
async fn api_self_get_profile(State(state): State<APIAppState>) -> Json<APISelfProfile> {
    let (username, avatar) = cmd_wrapper::get_local_user_profile(state).await;
    Json(APISelfProfile { username, avatar })
}

#[utoipa::path(
    post,
    path = "/self/profile",
    tag = "Self",
    request_body = APISelfProfile,
    responses(
        (status = 200, description = "Profile updated")
    )
)]
async fn api_self_post_profile(
    State(state): State<APIAppState>,
    Json(APISelfProfile { username, avatar }): Json<APISelfProfile>,
) -> Result<()> {
    cmd_wrapper::broadcast_profile(state, username, avatar)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/profile",
    tag = "Contacts",
    request_body = APISelfProfile,
    responses(
        (status = 200, description = "Shared profile")
    )
)]
async fn api_contact_share_profile(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APISelfProfile { username, avatar }): Json<APISelfProfile>,
) -> Result<()> {
    cmd_wrapper::share_profile(state, username, avatar, contact_id)
        .await
        .map_err(Into::into)
}

#[derive(Deserialize, ToSchema)]
struct APIFile {
    path: String,
}
#[derive(Serialize, ToSchema)]
struct APIFileDetails {
    name: String,
    size: u64,
}

#[utoipa::path(
    post,
    path = "/files/{contact_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID")
    ),
    request_body = APIFile,
    responses(
        (status = 200, description = "Sent file", body = APIFileDetails)
    )
)]
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

#[utoipa::path(
    post,
    path = "/files/{contact_id}/{offer_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("offer_id" = String, Path, description = "Offer ID"),
    ),
    request_body = APIFile,
    responses(
        (status = 200, description = "Accepted file")
    )
)]
async fn api_files_accept(
    State(state): State<APIAppState>,
    Path((contact_id, offer_id)): Path<(String, String)>,
    Json(APIFile { path }): Json<APIFile>,
) -> Result<()> {
    cmd_wrapper::accept_file_offer(state, contact_id, offer_id, path)
        .await
        .map_err(Into::into)
}
