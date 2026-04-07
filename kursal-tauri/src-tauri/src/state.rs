use kursal_core::{
    api::{AppEvent, CoreCommand},
    network::NetworkManager,
    storage::SharedDatabase,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};

pub struct AppState {
    pub db: SharedDatabase,
    pub network: Arc<Mutex<NetworkManager>>,
    pub app_event_tx: mpsc::Sender<AppEvent>,
    pub core_cmd_tx: mpsc::Sender<CoreCommand>,
    pub pending_nearby: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}
