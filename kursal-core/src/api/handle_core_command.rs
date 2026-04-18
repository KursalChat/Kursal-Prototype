use crate::{
    KursalError,
    api::{
        AppEvent, CoreCommand,
        file_transfers::{FileIncomingEntry, FileReceiveEntry, FileTransferEntry},
        send_message,
    },
    contacts::Contact,
    crypto::stream::derive_stream_key,
    first_contact::{
        ltc::LtcPayload,
        nearby::nearby_connect,
        otp::{fetch_otp, publish_otp},
    },
    identity::{TransportIdentity, UserId},
    messaging::{
        StoredMessage,
        enums::{
            FileAccept, FileOffer, KursalMessage, MessageDelete, MessageEdit, MessageId,
            ProfileInfo, ReactionAdd, ReactionRemove, TextMessage,
        },
    },
    network::{NetworkManager, rendezvous::publish_rendezvous, swarm::FILE_CHUNK_SIZE},
    storage::{SharedDatabase, TABLE_FILE_TRANSFERS, file::KursalFile, get_timestamp_secs},
};
use rand::{TryRngCore, rngs::OsRng};
use std::{path::Path, sync::Arc};
use tokio::sync::{Mutex, mpsc};

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
                .map(KursalFile::LtcPayload)
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
            method,
            reply,
        } => {
            let (mdns_transport, bt_transport, cmd_tx) = {
                let net = network.lock().await;
                let cmd_tx = net.primary.cmd_tx.clone();

                (net.mdns_transport.clone(), net.bt_transport.clone(), cmd_tx)
            };

            let result = if method == "mdns" {
                nearby_connect(
                    &peer_id,
                    &session_name,
                    &*mdns_transport,
                    db,
                    &app_event_tx,
                    &cmd_tx,
                )
                .await
            } else {
                nearby_connect(
                    &peer_id,
                    &session_name,
                    &*bt_transport,
                    db,
                    &app_event_tx,
                    &cmd_tx,
                )
                .await
            };
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

        CoreCommand::SendTypingIndicator { contact_id, reply } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let msg = KursalMessage::Typing;
                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                send_message(msg, &contact, db, &cmd_tx).await?;

                Ok(())
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

        CoreCommand::DeleteMessage {
            contact_id,
            message_id,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let msg = KursalMessage::MessageDelete(MessageDelete {
                    target_id: MessageId(message_id_bytes),
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let _msg_id = send_message(msg, &contact, db, &cmd_tx).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::EditMessage {
            contact_id,
            message_id,
            new_content,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let now = get_timestamp_secs()?;

                let msg = KursalMessage::MessageEdit(MessageEdit {
                    target_id: MessageId(message_id_bytes),
                    edited_at: now,
                    new_content,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let _msg_id = send_message(msg, &contact, db, &cmd_tx).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::ReactionAdd {
            contact_id,
            message_id,
            emoji,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let now = get_timestamp_secs()?;

                let msg = KursalMessage::ReactionAdd(ReactionAdd {
                    target_id: MessageId(message_id_bytes),
                    timestamp: now,
                    emoji,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let _msg_id = send_message(msg, &contact, db, &cmd_tx).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::ReactionRemove {
            contact_id,
            message_id,
            emoji,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let message_id_bytes: [u8; 16] = hex::decode(&message_id)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid message id length".to_string()))?;

                let msg = KursalMessage::ReactionRemove(ReactionRemove {
                    target_id: MessageId(message_id_bytes),
                    emoji,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let _msg_id = send_message(msg, &contact, db, &cmd_tx).await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::SendFileOffer {
            contact_id,
            file_path,
            reply,
        } => {
            let result = async {
                let user_id_bytes: [u8; 32] = hex::decode(&contact_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid contact id length".into()))?;

                let contact = Contact::load(&*db.0.lock().await, &UserId(user_id_bytes))?
                    .ok_or_else(|| KursalError::Storage("Contact not found".into()))?;

                let metadata = std::fs::metadata(&file_path).map_err(KursalError::Io)?;

                if !metadata.is_file() {
                    return Err(KursalError::Storage(
                        "Cannot transfer folders or symlinks".to_string(),
                    ));
                }

                let filename = Path::new(&file_path)
                    .file_name()
                    .ok_or(KursalError::Storage("File name not found".to_string()))?
                    .to_string_lossy()
                    .to_string();
                let size_bytes = metadata.len();

                let offer_id = MessageId::new();

                let mut my_random = [0u8; 32];
                OsRng
                    .try_fill_bytes(&mut my_random)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?;

                let entry = FileTransferEntry {
                    path: file_path,
                    my_random,
                };

                db.0.lock().await.raw_write(
                    TABLE_FILE_TRANSFERS,
                    &format!("{contact_id}:{}", hex::encode(offer_id.0)),
                    &entry
                        .serialize()
                        .map_err(|err| KursalError::Storage(err.to_string()))?,
                )?;

                let msg = KursalMessage::FileOffer(FileOffer {
                    id: offer_id,
                    filename,
                    size_bytes,
                    random: my_random,
                });

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                let msg_id = send_message(msg, &contact, db, &cmd_tx)
                    .await?
                    .expect("Text message always has an id");

                Ok((msg_id, size_bytes))
            }
            .await;

            reply.send(result).ok();
        }

        CoreCommand::AcceptFileOffer {
            contact_id,
            offer_id,
            save_path,
            reply,
        } => {
            let result = async {
                let entry_bytes =
                    db.0.lock()
                        .await
                        .raw_read(TABLE_FILE_TRANSFERS, &format!("{contact_id}:{offer_id}"))?
                        .ok_or(KursalError::Storage(
                            "Could not find file offer".to_string(),
                        ))?;

                let entry = FileIncomingEntry::deserialize(&entry_bytes)?;

                let file = std::fs::File::create(&save_path).map_err(KursalError::Io)?;
                file.set_len(entry.file_size).map_err(KursalError::Io)?;

                let chunk_count = usize::try_from(entry.file_size.div_ceil(FILE_CHUNK_SIZE as u64))
                    .map_err(|err| KursalError::Storage(err.to_string()))?;
                let bitset_len = chunk_count.div_ceil(8);

                let mut my_random = [0u8; 32];
                OsRng
                    .try_fill_bytes(&mut my_random)
                    .map_err(|err| KursalError::Crypto(err.to_string()))?;

                let key = derive_stream_key(entry.their_random, my_random);

                let updated_entry = FileReceiveEntry {
                    key,
                    file_size: entry.file_size,
                    save_path: save_path.clone(),
                    received_chunks: vec![0u8; bitset_len],
                };
                db.0.lock().await.raw_write(
                    TABLE_FILE_TRANSFERS,
                    &format!("{contact_id}:{offer_id}"),
                    &updated_entry.serialize()?,
                )?;

                let contact = Contact::load_all(&*db.0.lock().await)?
                    .into_iter()
                    .find(|c| hex::encode(c.user_id.0) == contact_id)
                    .ok_or_else(|| KursalError::Identity("Unknown contact".to_string()))?;

                let offer_id_bytes: [u8; 16] = hex::decode(&offer_id)
                    .map_err(|e| KursalError::Crypto(e.to_string()))?
                    .try_into()
                    .map_err(|_| KursalError::Crypto("Invalid offer id length".into()))?;

                let cmd_tx = network.lock().await.primary.cmd_tx.clone();

                send_message(
                    KursalMessage::FileAccept(FileAccept {
                        offer_id: MessageId(offer_id_bytes),
                        random: my_random,
                    }),
                    &contact,
                    db.clone(),
                    &cmd_tx,
                )
                .await?;

                Ok(())
            }
            .await;

            reply.send(result).ok();
        }
    }
}
