use crate::{
    KursalError, Result,
    first_contact::nearby::{BtEvent, NearbyBeacon, NearbyMessage, NearbyTransport},
    network::swarm::SwarmCommand,
};
use ble_peripheral_rust::{
    Peripheral as PeripheralAd, PeripheralImpl,
    gatt::{
        characteristic::Characteristic as AdCharacteristic,
        peripheral_event::{
            PeripheralEvent, ReadRequestResponse, RequestResponse, WriteRequestResponse,
        },
        properties::{AttributePermission, CharacteristicProperty},
        service::Service,
    },
};
use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral as PlatformPeripheral};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x4b75_7273_616c_0001_0000_0000_0000_0001);
const CHAR_UUID: Uuid = Uuid::from_u128(0x4b75_7273_616c_0001_0000_0000_0000_0002);
const POWER_POLL_INTERVAL: Duration = Duration::from_millis(250);
const POWER_POLL_MAX_ATTEMPTS: u32 = 60;

const FRAG_HEADER_LEN: usize = 12;
const ATT_OVERHEAD: usize = 3;
const DEFAULT_CHUNK_SIZE: usize = 180;
const MIN_CHUNK_SIZE: usize = 8;
const MAX_CHUNK_SIZE: usize = 500;
const REASSEMBLY_TTL: Duration = Duration::from_secs(60);
const PEER_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const PEER_WAIT_POLL: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct PartialMsg {
    total: usize,
    fragments: Vec<Option<Vec<u8>>>,
    received: usize,
    created_at: Instant,
}

type ReassemblyMap = Arc<Mutex<HashMap<(String, u64), PartialMsg>>>;

#[derive(Serialize, Deserialize)]
struct BtWireMessage {
    from_peer_id: String,
    msg: NearbyMessage,
}

struct BtPeer {
    peripheral: PlatformPeripheral,
    write_char: Characteristic,
}

struct AdvState {
    peripheral: PeripheralAd,
    service_added: bool,
    advertising: bool,
}

struct ScanState {
    central: Adapter,
    scanning: bool,
}

pub struct BTTransport {
    pub cmd_tx: mpsc::Sender<SwarmCommand>,
    pub my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    pub pending_handshakes: Arc<Mutex<HashMap<String, mpsc::Sender<NearbyMessage>>>>,
    pub bt_event_tx: mpsc::Sender<BtEvent>,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    adv: Arc<Mutex<Option<AdvState>>>,
    scan: Arc<Mutex<Option<ScanState>>>,
    reassembly: ReassemblyMap,
}

impl BTTransport {
    pub fn new(
        cmd_tx: mpsc::Sender<SwarmCommand>,
        my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
        bt_event_tx: mpsc::Sender<BtEvent>,
    ) -> Self {
        Self {
            cmd_tx,
            my_beacon,
            pending_handshakes: Arc::new(Mutex::new(HashMap::new())),
            bt_event_tx,
            peers: Arc::new(Mutex::new(HashMap::new())),
            adv: Arc::new(Mutex::new(None)),
            scan: Arc::new(Mutex::new(None)),
            reassembly: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl NearbyTransport for BTTransport {
    async fn start(&self, beacon: NearbyBeacon) {
        log::info!("[bt] starting bluetooth transport");

        if let Err(err) =
            start_scanner(&self.scan, self.peers.clone(), self.bt_event_tx.clone()).await
        {
            log::error!("[bt] scanner start: {err}");
        }

        if let Err(err) = start_advertiser(
            &self.adv,
            beacon,
            self.my_beacon.clone(),
            self.bt_event_tx.clone(),
            self.reassembly.clone(),
        )
        .await
        {
            log::error!("[bt] advertiser start: {err}");
        }
    }

    async fn stop(&self) {
        if let Some(state) = self.scan.lock().await.as_mut()
            && state.scanning
        {
            if let Err(err) = state.central.stop_scan().await {
                log::warn!("[bt] stop scan: {err}");
            } else {
                state.scanning = false;
            }
        }

        if let Some(state) = self.adv.lock().await.as_mut()
            && state.advertising
        {
            if let Err(err) = state.peripheral.stop_advertising().await {
                log::warn!("[bt] stop advertising: {err}");
            } else {
                state.advertising = false;
            }
        }

        let drained = {
            let mut p = self.peers.lock().await;
            std::mem::take(&mut *p)
        };
        for (_, peer) in drained {
            let _ = peer.peripheral.disconnect().await;
        }
        log::info!("[bt] stopped");
    }

    async fn send(&self, peer_id: &str, msg: NearbyMessage) -> Result<()> {
        let my_beacon = self
            .my_beacon
            .lock()
            .await
            .clone()
            .ok_or_else(|| KursalError::Network("bt: no active beacon".into()))?;

        let wire = BtWireMessage {
            from_peer_id: my_beacon.peer_id,
            msg,
        };
        let bytes = bincode::serialize(&wire).map_err(|e| KursalError::Storage(e.to_string()))?;

        let (peripheral, write_char) = wait_for_peer(&self.peers, peer_id).await?;

        let mtu = peripheral.mtu() as usize;
        let chunk_size = if mtu > 23 {
            mtu.saturating_sub(FRAG_HEADER_LEN + ATT_OVERHEAD)
                .clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE)
        } else {
            DEFAULT_CHUNK_SIZE
        };

        let total_chunks = bytes.chunks(chunk_size).count();
        if total_chunks > u16::MAX as usize {
            return Err(KursalError::Network(format!(
                "bt: message too large ({} chunks)",
                total_chunks
            )));
        }
        let total = total_chunks as u16;
        let msg_id: u64 = rand::random();

        for (i, chunk) in bytes.chunks(chunk_size).enumerate() {
            let mut frame = Vec::with_capacity(FRAG_HEADER_LEN + chunk.len());
            frame.extend_from_slice(&msg_id.to_be_bytes());
            frame.extend_from_slice(&(i as u16).to_be_bytes());
            frame.extend_from_slice(&total.to_be_bytes());
            frame.extend_from_slice(chunk);

            peripheral
                .write(&write_char, &frame, WriteType::WithResponse)
                .await
                .map_err(|e| KursalError::Network(e.to_string()))?;
        }

        Ok(())
    }

    async fn register_handshake(&self, peer_id: &str) -> mpsc::Receiver<NearbyMessage> {
        let (tx, rx) = mpsc::channel::<NearbyMessage>(8);

        self.pending_handshakes
            .lock()
            .await
            .insert(peer_id.to_string(), tx);

        rx
    }

    async fn unregister_handshake(&self, peer_id: &str) {
        self.pending_handshakes.lock().await.remove(peer_id);
    }
}

async fn wait_for_peer(
    peers: &Arc<Mutex<HashMap<String, BtPeer>>>,
    peer_id: &str,
) -> Result<(PlatformPeripheral, Characteristic)> {
    let start = Instant::now();
    loop {
        {
            let guard = peers.lock().await;
            if let Some(peer) = guard.get(peer_id) {
                return Ok((peer.peripheral.clone(), peer.write_char.clone()));
            }
        }
        if start.elapsed() >= PEER_WAIT_TIMEOUT {
            return Err(KursalError::Network(format!(
                "bt: peer {peer_id} not connected"
            )));
        }
        tokio::time::sleep(PEER_WAIT_POLL).await;
    }
}

async fn start_scanner(
    scan: &Arc<Mutex<Option<ScanState>>>,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
) -> Result<()> {
    let err = |s: String| KursalError::Network(s);

    let mut guard = scan.lock().await;
    if guard.is_none() {
        let manager = Manager::new()
            .await
            .map_err(|e| err(format!("bt manager: {e}")))?;
        let central = manager
            .adapters()
            .await
            .map_err(|e| err(format!("bt adapters: {e}")))?
            .into_iter()
            .next()
            .ok_or_else(|| err("no bluetooth adapter".into()))?;

        let events = central
            .events()
            .await
            .map_err(|e| err(format!("bt events: {e}")))?;

        tokio::spawn(run_scanner_events(
            events,
            central.clone(),
            peers,
            bt_event_tx,
        ));

        *guard = Some(ScanState {
            central,
            scanning: false,
        });
    }

    let state = guard.as_mut().unwrap();

    if state.scanning {
        let _ = state.central.stop_scan().await;
        state.scanning = false;
    }

    state
        .central
        .start_scan(ScanFilter {
            services: vec![SERVICE_UUID],
        })
        .await
        .map_err(|e| err(format!("start scan: {e}")))?;
    state.scanning = true;

    log::info!("[bt] scanner started");
    Ok(())
}

async fn run_scanner_events(
    events: std::pin::Pin<Box<dyn futures::Stream<Item = CentralEvent> + Send>>,
    central: Adapter,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
) {
    let inflight: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    futures::pin_mut!(events);

    while let Some(event) = events.next().await {
        let id = match event {
            CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => id,
            _ => continue,
        };
        let bt_id = id.to_string();

        if peers_contains_bt_id(&peers, &bt_id).await {
            continue;
        }
        if !inflight.lock().await.insert(bt_id.clone()) {
            continue;
        }

        let peripheral = match central.peripheral(&id).await {
            Ok(p) => p,
            Err(_) => {
                inflight.lock().await.remove(&bt_id);
                continue;
            }
        };

        log::info!("[bt] kursal peripheral {bt_id} — handshaking");
        let peers_clone = peers.clone();
        let tx_clone = bt_event_tx.clone();
        let inflight_clone = inflight.clone();
        tokio::spawn(async move {
            if let Err(err) = handshake_with_peer(peripheral, peers_clone, tx_clone).await {
                log::warn!("[bt] handshake {bt_id}: {err}");
            }
            inflight_clone.lock().await.remove(&bt_id);
        });
    }

    log::warn!("[bt] scanner event loop exited");
}

async fn peers_contains_bt_id(peers: &Arc<Mutex<HashMap<String, BtPeer>>>, bt_id: &str) -> bool {
    let guard = peers.lock().await;
    guard
        .values()
        .any(|p| p.peripheral.id().to_string() == bt_id)
}

async fn handshake_with_peer(
    peripheral: PlatformPeripheral,
    peers: Arc<Mutex<HashMap<String, BtPeer>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
) -> Result<()> {
    let bt_err = |e: btleplug::Error| KursalError::Network(e.to_string());

    if !peripheral.is_connected().await.map_err(bt_err)? {
        peripheral.connect().await.map_err(bt_err)?;
    }
    peripheral.discover_services().await.map_err(bt_err)?;

    let chars = peripheral.characteristics();
    let write_char = chars
        .iter()
        .find(|c| c.uuid == CHAR_UUID)
        .ok_or_else(|| KursalError::Network("bt: kursal char missing".into()))?
        .clone();

    let beacon_bytes = peripheral.read(&write_char).await.map_err(bt_err)?;
    let beacon = NearbyBeacon::deserialize(&beacon_bytes)?;
    let peer_id = beacon.peer_id.clone();
    log::info!(
        "[bt] got beacon from {peer_id} session={:?}",
        beacon.session_name
    );

    peers.lock().await.insert(
        peer_id.clone(),
        BtPeer {
            peripheral,
            write_char,
        },
    );

    bt_event_tx
        .send(BtEvent::Beacon { peer_id, beacon })
        .await
        .map_err(|e| KursalError::Network(e.to_string()))?;

    Ok(())
}

async fn start_advertiser(
    adv: &Arc<Mutex<Option<AdvState>>>,
    beacon: NearbyBeacon,
    my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
    reassembly: ReassemblyMap,
) -> Result<()> {
    let err = |s: String| KursalError::Network(s);

    let mut guard = adv.lock().await;
    if guard.is_none() {
        let (event_tx, event_rx) = mpsc::channel::<PeripheralEvent>(256);

        let mut peripheral = PeripheralAd::new(event_tx)
            .await
            .map_err(|e| err(format!("peripheral init: {e}")))?;

        if !wait_powered(&mut peripheral).await {
            return Err(err("bluetooth not powered".into()));
        }

        tokio::spawn(run_peripheral_events(
            event_rx,
            my_beacon,
            bt_event_tx,
            reassembly,
        ));

        *guard = Some(AdvState {
            peripheral,
            service_added: false,
            advertising: false,
        });
    }

    let state = guard.as_mut().unwrap();

    if !state.service_added {
        let service = kursal_service();
        state
            .peripheral
            .add_service(&service)
            .await
            .map_err(|e| err(format!("add service: {e}")))?;
        state.service_added = true;
    }

    if state.advertising {
        state
            .peripheral
            .stop_advertising()
            .await
            .map_err(|e| err(format!("stop advertising: {e}")))?;
        state.advertising = false;
    }

    let ad_name = beacon.session_name.clone();
    state
        .peripheral
        .start_advertising(&ad_name, &[SERVICE_UUID])
        .await
        .map_err(|e| err(format!("start advertising: {e}")))?;
    state.advertising = true;

    log::info!("[bt] advertising as {ad_name:?}");
    Ok(())
}

fn kursal_service() -> Service {
    Service {
        uuid: SERVICE_UUID,
        primary: true,
        characteristics: vec![AdCharacteristic {
            uuid: CHAR_UUID,
            properties: vec![CharacteristicProperty::Read, CharacteristicProperty::Write],
            permissions: vec![
                AttributePermission::Readable,
                AttributePermission::Writeable,
            ],
            value: None,
            descriptors: vec![],
        }],
    }
}

async fn wait_powered(peripheral: &mut PeripheralAd) -> bool {
    for _ in 0..POWER_POLL_MAX_ATTEMPTS {
        match peripheral.is_powered().await {
            Ok(true) => return true,
            Ok(false) => {}
            Err(err) => {
                log::error!("[bt] power poll: {err}");
                return false;
            }
        }
        tokio::time::sleep(POWER_POLL_INTERVAL).await;
    }
    log::warn!("[bt] bluetooth never powered on");
    false
}

async fn run_peripheral_events(
    mut event_rx: mpsc::Receiver<PeripheralEvent>,
    my_beacon: Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: mpsc::Sender<BtEvent>,
    reassembly: ReassemblyMap,
) {
    while let Some(event) = event_rx.recv().await {
        handle_peripheral_event(event, &my_beacon, &bt_event_tx, &reassembly).await;
    }
}

async fn handle_peripheral_event(
    event: PeripheralEvent,
    my_beacon: &Arc<Mutex<Option<NearbyBeacon>>>,
    bt_event_tx: &mpsc::Sender<BtEvent>,
    reassembly: &ReassemblyMap,
) {
    match event {
        PeripheralEvent::StateUpdate { is_powered } => {
            log::info!("[bt] power state: {is_powered:?}");
        }
        PeripheralEvent::CharacteristicSubscriptionUpdate { .. } => {}
        PeripheralEvent::ReadRequest { responder, .. } => {
            let value = match my_beacon.lock().await.as_ref() {
                Some(b) => b.serialize().unwrap_or_default(),
                None => Vec::new(),
            };
            let _ = responder.send(ReadRequestResponse {
                value,
                response: RequestResponse::Success,
            });
        }
        PeripheralEvent::WriteRequest {
            request,
            value,
            responder,
            ..
        } => {
            let _ = responder.send(WriteRequestResponse {
                response: RequestResponse::Success,
            });

            if let Some(bytes) = push_fragment(reassembly, &request.client, &value).await {
                match bincode::deserialize::<BtWireMessage>(&bytes) {
                    Ok(wire) => {
                        let _ = bt_event_tx
                            .send(BtEvent::Message {
                                from_peer_id: wire.from_peer_id,
                                msg: wire.msg,
                            })
                            .await;
                    }
                    Err(err) => {
                        log::warn!("[bt] bad wire message: {err}");
                    }
                }
            }
        }
    }
}

async fn push_fragment(reassembly: &ReassemblyMap, client: &str, frame: &[u8]) -> Option<Vec<u8>> {
    if frame.len() < FRAG_HEADER_LEN {
        log::warn!("[bt] fragment shorter than header ({} bytes)", frame.len());
        return None;
    }
    let msg_id = u64::from_be_bytes(frame[0..8].try_into().ok()?);
    let seq = u16::from_be_bytes(frame[8..10].try_into().ok()?) as usize;
    let total = u16::from_be_bytes(frame[10..12].try_into().ok()?) as usize;
    let data = &frame[FRAG_HEADER_LEN..];

    if total == 0 || seq >= total {
        log::warn!("[bt] fragment seq/total invalid (seq={seq}, total={total})");
        return None;
    }

    let mut map = reassembly.lock().await;

    let now = Instant::now();
    map.retain(|_, v| now.duration_since(v.created_at) < REASSEMBLY_TTL);

    let key = (client.to_string(), msg_id);
    let partial = map.entry(key.clone()).or_insert_with(|| PartialMsg {
        total,
        fragments: vec![None; total],
        received: 0,
        created_at: now,
    });

    if partial.total != total || partial.fragments.len() != total {
        log::warn!("[bt] fragment total mismatch for msg {msg_id}");
        map.remove(&key);
        return None;
    }
    if partial.fragments[seq].is_some() {
        return None;
    }

    partial.fragments[seq] = Some(data.to_vec());
    partial.received += 1;

    if partial.received < partial.total {
        return None;
    }

    let done = map.remove(&key)?;
    let mut out = Vec::new();
    for frag in done.fragments.into_iter() {
        out.extend(frag?);
    }
    Some(out)
}
