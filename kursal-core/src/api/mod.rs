use crate::{
    KursalError, Result,
    contacts::Contact,
    crypto::{
        dilithium::{dilithium_sign, dilithium_verify},
        messages::{message_receive, message_send},
    },
    first_contact::{
        WireMessage,
        ltc::LtcPayload,
        nearby::nearby_connect,
        otp::{fetch_otp, handle_otp_response, publish_otp},
    },
    identity::{TransportIdentity, UserId},
    messaging::{
        StoredMessage, StoredReaction,
        enums::{
            DeliveryReceipt, Direction, KursalMessage, MessageId, MessageStatus, ProfileInfo,
            TextMessage,
        },
    },
    network::{
        NetworkManager,
        rendezvous::publish_rendezvous,
        swarm::{SwarmCommand, str_to_multiaddr},
    },
    storage::{SharedDatabase, TABLE_SETTINGS, file::KursalFile, get_timestamp_secs},
};
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use std::{str::FromStr, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};
use zeroize::Zeroize;

pub mod state;

pub enum ConnectionStatus {
    Connecting,
    Relay,
    HolePunch,
    Direct,
    Disconnected,
}

pub enum AppEvent {
    MessageReceived {
        contact_id: UserId,
        message: StoredMessage,
    },
    DeliveryConfirmed {
        contact_id: UserId,
        message_id: MessageId,
    },
    MessageEdited {
        contact_id: UserId,
        message_id: MessageId,
        new_content: String,
    },
    MessageDeleted {
        contact_id: UserId,
        message_id: MessageId,
    },
    ReactionAdded {
        contact_id: UserId,
        message_id: MessageId,
        emoji: String,
    },
    ReactionRemoved {
        contact_id: UserId,
        message_id: MessageId,
        emoji: String,
    },
    ContactAdded {
        contact: Contact,
    },
    ContactUpdated {
        contact: Contact,
    },
    ContactRemoved {
        contact_id: UserId,
    },
    PeerIdRotated {
        new_addresses: Vec<String>,
    },
    ConnectionChange {
        contact_id: UserId,
        status: ConnectionStatus,
    },
    LTCExpiringSoon {
        hours_remaining: u32,
    },
    NearbyRequest {
        peer_id: String,
        session_name: String,
        decision_tx: oneshot::Sender<bool>,
    },
}

pub async fn send_message(
    content: KursalMessage,
    contact: &Contact,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<Option<MessageId>> {
    let serialized = content.serialize()?;
    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DeviceId::new(1u8).unwrap());

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

    let Some(msg_id) = content.message_id() else {
        return Ok(None);
    };

    // dont save some kind of messages (some that returns the message_id but should not save)
    if matches!(
        content,
        KursalMessage::DeliveryReceipt(_)
            | KursalMessage::FileChunk(_)
            | KursalMessage::CallSignal(_)
            | KursalMessage::Reaction(_)
    ) {
        return Ok(Some(msg_id));
    }

    let now = get_timestamp_secs()?;
    let stored = StoredMessage {
        id: msg_id,
        status: MessageStatus::Sending,
        direction: Direction::Sent,
        timestamp: now,
        contact_id: contact.user_id.clone(),
        payload: content,
        raw_ciphertext: Some(message),
        deleted: false,
        edited: false,
        reactions: Vec::with_capacity(0),
    };

    stored.save(&*db.0.lock().await)?;

    Ok(Some(msg_id))
}

pub async fn handle_incoming(
    from: PeerId,
    ciphertext: Vec<u8>,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let peer_id_str = from.to_base58();

    let encrypted_payload = match bincode::deserialize::<WireMessage>(&ciphertext) {
        Ok(WireMessage::ContactResponse(response)) => {
            let _ = handle_otp_response(response, db.clone(), cmd_tx, event_tx).await;
            return Ok(());
        }
        Ok(WireMessage::KeyRotationAnnouncement(rotation)) => {
            // TODO: some kind of check? maybe? maybe not needed because signature
            // if rotation.new_peer_id != peer_id_str {
            //     log::debug!("ignoring keyrotation because peers don't match");
            //     return Ok(());
            // }

            // if timestamp older than 10 minutes
            let now = get_timestamp_secs()?;
            if rotation.timestamp + 600 < now || now + 60 < rotation.timestamp {
                log::debug!("ignoring {peer_id_str}'s keyrotation because timestamp is too old");
                return Ok(());
            }

            let all_contacts = Contact::load_all(&*db.clone().0.lock().await)?;
            let mut contact = all_contacts
                .into_iter()
                .find(|c| c.peer_id == rotation.old_peer_id)
                .ok_or_else(|| KursalError::Identity("unknown peer".to_string()))?;

            // still accept rotation if blocked

            let rotation_data = [
                rotation.old_peer_id.as_bytes(),
                rotation.new_peer_id.as_bytes(),
                rotation.new_addresses.join(",").as_bytes(),
                &rotation.timestamp.to_be_bytes(),
            ]
            .concat();

            let is_valid = dilithium_verify(
                &contact.dilithium_pub_key,
                &rotation_data,
                &rotation.signature,
            )?;

            if !is_valid {
                log::warn!(
                    "KeyRotationAnnouncement signature verification failed: contact_id={:?}",
                    contact.user_id
                );
                return Err(KursalError::Crypto(
                    "Invalid key rotation signature".to_string(),
                ));
            }

            contact.known_addresses = rotation.new_addresses.clone();
            contact.peer_id = rotation.new_peer_id.clone();
            contact.save(&*db.clone().0.lock().await)?;

            for addr in &rotation.new_addresses {
                if let Ok(ma) = addr.parse() {
                    let _ = cmd_tx.send(SwarmCommand::Dial(ma)).await;
                }
            }

            event_tx
                .send(AppEvent::ContactUpdated { contact })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            return Ok(());
        }
        Ok(WireMessage::Encrypted(data)) => data,
        Err(_e) => {
            // log::debug!("Rejected invalid WireMessage from {}: {}", peer_id_str, e);
            return Ok(());
        }
    };

    let mut contact = Contact::load_all(&*db.clone().0.lock().await)?
        .into_iter()
        .find(|c| c.peer_id == peer_id_str)
        .ok_or_else(|| KursalError::Identity("unknown peer".to_string()))?;

    if contact.blocked {
        return Err(KursalError::Identity("blocked peer".to_string()));
    }

    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DeviceId::new(1u8).unwrap());
    let now = get_timestamp_secs()?;

    let received = message_receive(db.clone(), &address, &encrypted_payload).await?;
    let kmessage = KursalMessage::deserialize(&received)?;

    match kmessage {
        KursalMessage::MessageEdit(ref edit) => {
            let mut message = StoredMessage::load(
                &*db.clone().0.lock().await,
                &contact.user_id,
                &edit.target_id,
            )?
            .ok_or(KursalError::Storage("Message not found".to_string()))?;

            if let KursalMessage::Text(ref mut t) = message.payload {
                t.content = edit.new_content.clone();
            }
            message.edited = true;
            message.save(&*db.clone().0.lock().await)?;

            event_tx
                .send(AppEvent::MessageEdited {
                    contact_id: contact.user_id,
                    message_id: edit.target_id,
                    new_content: edit.new_content.clone(),
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;
        }

        KursalMessage::MessageDelete(ref del) => {
            let mut message = StoredMessage::load(
                &*db.clone().0.lock().await,
                &contact.user_id,
                &del.target_id,
            )?
            .ok_or(KursalError::Storage("Message not found".to_string()))?;

            if let KursalMessage::Text(ref mut t) = message.payload {
                t.content = String::with_capacity(0);
            }
            message.deleted = true;
            message.raw_ciphertext = None;

            message.save(&*db.clone().0.lock().await)?;

            event_tx
                .send(AppEvent::MessageDeleted {
                    contact_id: contact.user_id,
                    message_id: del.target_id,
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;
        }

        KursalMessage::ReactionRemove(ref r) => {
            let mut message =
                StoredMessage::load(&*db.clone().0.lock().await, &contact.user_id, &r.target_id)?
                    .ok_or(KursalError::Storage("Message not found".to_string()))?;

            message
                .reactions
                .retain(|rx| !(rx.emoji == r.emoji && rx.user_id == contact.user_id));
            message.save(&*db.clone().0.lock().await)?;

            event_tx
                .send(AppEvent::ReactionRemoved {
                    contact_id: contact.user_id,
                    message_id: r.target_id,
                    emoji: r.emoji.clone(),
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;
        }

        KursalMessage::Reaction(ref r) => {
            if emojis::get(&r.emoji).is_none() {
                log::warn!("Rejected invalid reaction emoji");
                return Ok(());
            }

            let mut target =
                StoredMessage::load(&*db.clone().0.lock().await, &contact.user_id, &r.target_id)?
                    .ok_or(KursalError::Storage("Target message not found".to_string()))?;

            target.reactions.push(StoredReaction {
                emoji: r.emoji.clone(),
                user_id: contact.user_id.clone(),
                timestamp: now,
            });
            target.save(&*db.clone().0.lock().await)?;

            event_tx
                .send(AppEvent::ReactionAdded {
                    contact_id: contact.user_id.clone(),
                    message_id: r.target_id,
                    emoji: r.emoji.clone(),
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;
        }

        KursalMessage::Text(_) | KursalMessage::FileOffer(_) => {
            let msg_id = kmessage
                .message_id()
                .expect("storable message always has an id");

            let stored = StoredMessage {
                id: msg_id,
                contact_id: contact.user_id.clone(),
                payload: kmessage,
                timestamp: now,
                direction: Direction::Received,
                status: MessageStatus::Delivered,
                raw_ciphertext: None,
                deleted: false,
                edited: false,
                reactions: Vec::with_capacity(0),
            };

            stored.save(&*db.clone().0.lock().await)?;

            event_tx
                .send(AppEvent::MessageReceived {
                    contact_id: contact.user_id.clone(),
                    message: stored,
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            // send delivery receipt
            let mut secret_key =
                db.0.lock()
                    .await
                    .raw_read(TABLE_SETTINGS, "dilithium_secret")?
                    .ok_or(KursalError::Storage("No dilithium-5 secret".to_string()))?;

            let signed = dilithium_sign(&secret_key, &encrypted_payload)?;
            secret_key.zeroize();

            let receipt = KursalMessage::DeliveryReceipt(DeliveryReceipt {
                message_id: msg_id,
                signature: signed,
            });

            send_message(receipt, &contact, db, cmd_tx).await?;
        }

        KursalMessage::DeliveryReceipt(receipt) => {
            let mut message = StoredMessage::load(
                &*db.clone().0.lock().await,
                &contact.user_id,
                &receipt.message_id,
            )?
            .ok_or(KursalError::Storage("Message not found".to_string()))?;

            let original_ciphertext = message.raw_ciphertext.clone().ok_or(
                KursalError::Storage("Raw ciphertext not stored".to_string()),
            )?;

            let signature = dilithium_verify(
                &contact.dilithium_pub_key,
                &original_ciphertext,
                &receipt.signature,
            )?;

            if signature {
                message.status = MessageStatus::Delivered;
                message.save(&*db.clone().0.lock().await)?;

                event_tx
                    .send(AppEvent::DeliveryConfirmed {
                        contact_id: contact.user_id,
                        message_id: message.id,
                    })
                    .await
                    .map_err(|err| KursalError::Network(err.to_string()))?;
            } else {
                log::warn!(
                    "Delivery receipt signature verification failed: message_id={:?} contact_id={:?}",
                    receipt.message_id,
                    contact.user_id
                );
            }
        }

        KursalMessage::FileChunk(_) | KursalMessage::CallSignal(_) => {
            // TODO: handle
        }

        KursalMessage::ProfileUpdate(profile) => {
            log::info!(
                "Received profile update: USERNAME: {} - AVATAR LEN: {}",
                profile.display_name,
                profile.avatar_bytes.as_ref().map(|e| e.len()).unwrap_or(0)
            );
            profile.validate()?;

            contact.display_name = profile.display_name;
            contact.avatar_bytes = profile.avatar_bytes;

            contact.save(&*db.0.lock().await)?;

            event_tx
                .send(AppEvent::ContactUpdated { contact })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;
        }
    }

    Ok(())
}

//

pub enum CoreCommand {
    PublishOtp {
        otp: String,
        reply: oneshot::Sender<Result<()>>,
    },
    FetchOtp {
        otp: String,
        reply: oneshot::Sender<Result<Contact>>,
    },
    ExportLtc {
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    ImportLtc {
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<Contact>>,
    },
    ConnectNearby {
        peer_id: String,
        session_name: String,
        reply: oneshot::Sender<Result<()>>,
    },
    SendText {
        contact_id: String,
        text: String,
        reply_to: Option<MessageId>,
        reply: oneshot::Sender<Result<MessageId>>,
    },
    RotatePeerId {
        reply: oneshot::Sender<Result<()>>,
    },
    RemoveContact {
        contact_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    DeleteLocalMessage {
        contact_id: String,
        message_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    ShareProfile {
        contact_id: String,
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<()>>,
    },
    BroadcastProfile {
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<()>>,
    },
}

pub async fn handle_core_command(
    cmd: CoreCommand,
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    app_event_tx: mpsc::Sender<AppEvent>,
) {
    match cmd {
        CoreCommand::PublishOtp { otp, reply } => {
            let net = network.lock().await;
            let result = publish_otp(&otp, db, &net).await;
            reply.send(result).ok();
        }

        CoreCommand::FetchOtp { otp, reply } => {
            let net = network.lock().await;
            let result = fetch_otp(&otp, db, &net).await;
            reply.send(result).ok();
        }

        CoreCommand::ExportLtc { reply } => {
            let net = network.lock().await;
            let result = LtcPayload::generate(db, &net)
                .await
                .and_then(|p| p.serialize())
                .map(|res| KursalFile::LtcPayload(res))
                .and_then(|p| p.serialize());

            reply.send(result).ok();
        }

        CoreCommand::ImportLtc { bytes, reply } => {
            let net = network.lock().await;

            let result = match KursalFile::deserialize(&bytes) {
                Ok(KursalFile::LtcPayload(bytes)) => LtcPayload::deserialize(&bytes),
                _ => Err(KursalError::Identity("Invalid file type".to_string())),
            };

            let result = match result {
                Ok(payload) => payload.import_ltc(db, &net).await,
                Err(e) => Err(e),
            };
            reply.send(result).ok();
        }

        CoreCommand::ConnectNearby {
            peer_id,
            session_name,
            reply,
        } => {
            let (transport, cmd_tx) = {
                let net = network.lock().await;
                let transport = match net.mdns_transport.clone() {
                    Some(t) => t,
                    None => {
                        reply
                            .send(Err(KursalError::Network("Nearby share not active".into())))
                            .ok();
                        return;
                    }
                };
                let cmd_tx = net.primary.cmd_tx.clone();
                (transport, cmd_tx)
            };

            let result = nearby_connect(
                &peer_id,
                &session_name,
                transport.as_ref(),
                db,
                &app_event_tx,
                &cmd_tx,
            )
            .await;
            reply.send(result).ok();
        }

        CoreCommand::SendText {
            contact_id,
            text,
            reply_to,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let now = get_timestamp_secs()?;

                let msg = KursalMessage::Text(TextMessage {
                    id: MessageId::new(),
                    content: text,
                    timestamp: now,
                    reply_to,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let msg_id = send_message(msg, &contact, db, &cmd_tx)
                    .await?
                    .expect("Text message always has an id");

                Ok(msg_id)
            }
            .await;

            reply.send(result).ok();
        }

        // TODO: if one fn fails then it should maybe rollback
        CoreCommand::RotatePeerId { reply } => {
            let result = async {
                let new_identity = TransportIdentity::generate();
                let new_peer_id = new_identity.peer_id.to_base58();

                let mut net = network.lock().await;
                let new_cmd_tx = net.start_rotation(new_identity).await?;
                drop(net);

                // wait to discover listening addrs and bootstrap relays
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;

                // broadcast through OLD swarm (which has the connections)
                let net = network.lock().await;
                net.broadcast_key_rotation(db.clone(), &net.primary.cmd_tx)
                    .await?;
                drop(net);

                // wait for peers to receive and process rotation
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                let mut net = network.lock().await;
                net.complete_rotation().await?;
                drop(net);

                publish_rendezvous(&*db.0.lock().await, new_peer_id, &new_cmd_tx).await
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::ShareProfile {
            contact_id,
            display_name,
            avatar_bytes,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let mut contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                contact.profile_shared = true;
                contact.save(&*db.0.lock().await)?;

                let msg = KursalMessage::ProfileUpdate(ProfileInfo {
                    display_name,
                    avatar_bytes,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                let _ = send_message(msg, &contact, db.clone(), &cmd_tx).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::BroadcastProfile {
            display_name,
            avatar_bytes,
            reply,
        } => {
            let result = async {
                let cmd_tx = network.lock().await.primary.cmd_tx.clone();
                let contacts = Contact::load_all(&*db.0.lock().await)?;

                for contact in contacts {
                    if contact.profile_shared {
                        let msg = KursalMessage::ProfileUpdate(ProfileInfo {
                            display_name: display_name.clone(),
                            avatar_bytes: avatar_bytes.clone(),
                        });

                        let _ = send_message(msg, &contact, db.clone(), &cmd_tx).await?;
                    }
                }

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::RemoveContact { contact_id, reply } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let user_id = UserId(user_id_bytes);

                Contact::delete(&*db.0.lock().await, &user_id)?;

                app_event_tx
                    .send(AppEvent::ContactRemoved {
                        contact_id: user_id,
                    })
                    .await
                    .map_err(|err| KursalError::Network(err.to_string()))?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::DeleteLocalMessage {
            contact_id,
            message_id,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;
                let user_id = UserId(user_id_bytes);

                let id_bytes: [u8; 16] = hex::decode(&message_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".into()))?;
                let msg_id = MessageId(id_bytes);

                StoredMessage::delete(&*db.0.lock().await, &user_id, &msg_id)?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }
    }
}

pub fn get_protocol_addr(user_id: [u8; 32]) -> ProtocolAddress {
    ProtocolAddress::new(hex::encode(user_id), DeviceId::new(1u8).unwrap())
}
