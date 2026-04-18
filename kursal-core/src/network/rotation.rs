use crate::{
    KursalError, Result,
    api::CoreCommand,
    contacts::Contact,
    crypto::dilithium::dilithium_sign,
    first_contact::{WireMessage, nearby::mdns::MdnsTransport},
    identity::TransportIdentity,
    messaging::enums::KeyRotation,
    network::{
        NetworkManager,
        dht::DHTRecord,
        rendezvous::RendezvousRecord,
        swarm::{SwarmCommand, SwarmHandle, get_listen_addrs, str_to_multiaddr},
    },
    storage::{
        SharedDatabase, TABLE_SETTINGS, get_dilithium_pub, get_local_identity_pub,
        get_timestamp_secs,
    },
};
use libp2p::PeerId;
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc, oneshot};
use zeroize::Zeroize;

impl NetworkManager {
    pub async fn start_rotation(
        &mut self,
        identity: TransportIdentity,
    ) -> Result<mpsc::Sender<SwarmCommand>> {
        // shutdown any secondary
        if let Some(ref secondary) = self.secondary {
            let _ = secondary.cmd_tx.send(SwarmCommand::Shutdown).await;
        }

        let secondary = SwarmHandle::spawn(
            identity,
            self.event_tx.clone(),
            self.primary.relay_server_enabled,
        )
        .await?;

        let cmd_tx = secondary.cmd_tx.clone();
        self.secondary = Some(secondary);

        Ok(cmd_tx)
    }

    pub async fn broadcast_key_rotation(
        &self,
        db: SharedDatabase,
        cmd_tx: &mpsc::Sender<SwarmCommand>,
    ) -> Result<()> {
        let contacts = Contact::load_all(&*db.clone().0.lock().await)?;

        let mut secret_key = db
            .clone()
            .0
            .lock()
            .await
            .raw_read(TABLE_SETTINGS, "dilithium_secret")?
            .ok_or(KursalError::Storage("No dilithium-5 secret".to_string()))?;

        let secondary = self.secondary.as_ref().ok_or(KursalError::Identity(
            "Could not access secondary identity".to_string(),
        ))?;

        let old_peer_id = self.primary.peer_id.to_base58();
        let new_peer_id = secondary.peer_id.to_base58();
        let new_addresses = get_listen_addrs(&secondary.cmd_tx).await?;
        let now = get_timestamp_secs()?;

        let rotation_data = [
            old_peer_id.as_bytes(),
            new_peer_id.as_bytes(),
            new_addresses.join(",").as_bytes(),
            &now.to_be_bytes(),
        ]
        .concat();
        let signature = dilithium_sign(&secret_key, &rotation_data)?;
        secret_key.zeroize();

        let rotation_msg = KeyRotation {
            old_peer_id,
            new_peer_id: new_peer_id.clone(),
            new_addresses: new_addresses.clone(),
            signature,
            timestamp: now,
        };

        let wire_msg = WireMessage::KeyRotationAnnouncement(rotation_msg);
        let serialized =
            bincode::serialize(&wire_msg).map_err(|e| KursalError::Storage(e.to_string()))?;

        for contact in contacts {
            let peer_id = PeerId::from_str(&contact.peer_id)
                .map_err(|_| KursalError::Network("Invalid peer id".to_string()))?;

            if let Err(ohno) = cmd_tx
                .send(SwarmCommand::SendMessage {
                    peer_id,
                    data: serialized.clone(),
                    addresses: str_to_multiaddr(&contact.known_addresses)?,
                })
                .await
            {
                log::warn!(
                    "Could not broadcast key rotation to {}: {}",
                    contact.peer_id,
                    ohno
                );
            }
        }

        Ok(())
    }

    pub async fn complete_rotation(&mut self) -> Result<()> {
        let _ = self.primary.cmd_tx.send(SwarmCommand::Shutdown).await;

        let new_primary = self
            .secondary
            .take()
            .ok_or_else(|| KursalError::Network("No secondary swarm".to_string()))?;
        self.primary = new_primary;

        // update mdns_transport to use new swarm's command channel
        self.mdns_transport = Arc::new(MdnsTransport::new(
            self.primary.cmd_tx.clone(),
            self.my_beacon.clone(),
        ));

        Ok(())
    }

    pub async fn spawn_rotation_scheduler(
        db: SharedDatabase,
        core_cmd_tx: mpsc::Sender<CoreCommand>,
        network: Arc<Mutex<NetworkManager>>,
    ) {
        loop {
            let secs = {
                let lock = db.0.lock().await;
                lock.raw_read(TABLE_SETTINGS, "rotation_interval_secs")
                    .ok()
                    .flatten()
                    .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                    .unwrap_or(30 * 60 * 60) // 30 hours
            };

            tokio::time::sleep(Duration::from_secs(secs.max(300))).await;

            let (reply_tx, reply_rx) = oneshot::channel();
            if core_cmd_tx
                .send(CoreCommand::RotatePeerId { reply: reply_tx })
                .await
                .is_err()
            {
                break;
            }

            if let Err(ohno) = reply_rx.await {
                log::error!("Failed to rotate peer ID: {}", ohno);
            }

            if let Err(ohno) = NetworkManager::rendezvous_publish(db.clone(), network.clone()).await
            {
                log::error!("Failed to publish rendezvous: {}", ohno);
            }
        }
    }

    pub async fn rendezvous_publish(
        db: SharedDatabase,
        network: Arc<Mutex<NetworkManager>>,
    ) -> Result<()> {
        let (lookup_bytes, secret_key_bytes) = {
            let db_lock = db.0.lock().await;

            let secret_key_bytes = db_lock
                .raw_read(TABLE_SETTINGS, "dilithium_secret")?
                .ok_or(KursalError::Crypto("No signing key stored".to_string()))?;

            (
                [
                    get_local_identity_pub(&db_lock)?,
                    get_dilithium_pub(&db_lock)?,
                ]
                .concat(),
                secret_key_bytes,
            )
        };

        let dht_key = Sha256::digest(lookup_bytes).to_vec();

        let (relay_addresses, peer_id) = {
            let net_lock = &network.lock().await;

            (
                get_listen_addrs(&net_lock.primary.cmd_tx).await?,
                net_lock.primary.peer_id.to_base58(),
            )
        };

        let timestamp = get_timestamp_secs()?;
        let record = RendezvousRecord {
            peer_id,
            relay_addresses,
        };

        let dht_record = DHTRecord::new(
            dht_key.clone(),
            secret_key_bytes,
            record.serialize()?,
            timestamp,
        )
        .await?;

        network
            .lock()
            .await
            .primary
            .cmd_tx
            .send(SwarmCommand::PublishDht {
                key: dht_key,
                value: dht_record.serialize()?,
            })
            .await
            .map_err(|err| KursalError::Network(err.to_string()))?;

        log::info!("published rendezvous record");

        Ok(())
    }

    pub async fn spawn_rendezvous_publisher(
        db: SharedDatabase,
        network: Arc<Mutex<NetworkManager>>,
    ) {
        loop {
            let secs = {
                let lock = db.0.lock().await;
                lock.raw_read(TABLE_SETTINGS, "rendezvous_interval_secs")
                    .ok()
                    .flatten()
                    .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                    .unwrap_or(12 * 60 * 60) // 12 hours
            };

            tokio::time::sleep(Duration::from_secs(secs)).await;

            if let Err(ohno) = NetworkManager::rendezvous_publish(db.clone(), network.clone()).await
            {
                log::error!("Failed to publish rendezvous: {}", ohno);
            }
        }
    }
}
