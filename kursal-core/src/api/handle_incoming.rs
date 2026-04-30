use crate::{
    KursalError, Result,
    api::{
        AppEvent,
        file_transfers::{
            FileIncomingEntry, FileReceiveEntry, FileTransferEntry, send_file_chunks,
        },
        send_message,
    },
    contacts::Contact,
    crypto::{
        dilithium::{dilithium_sign, dilithium_verify},
        messages::message_receive,
        stream::stream_decrypt,
    },
    first_contact::{WireMessage, handle_fc_response},
    messaging::{
        StoredMessage, StoredReaction,
        enums::{DeliveryReceipt, Direction, KursalMessage, MessageId, MessageStatus},
    },
    network::swarm::{FILE_CHUNK_SIZE, MAX_MESSAGE_SIZE, NetworkEvent, SwarmCommand},
    storage::{
        SharedDatabase, TABLE_FILE_TRANSFERS, TABLE_SETTINGS,
        filetransfer::{
            get_auto_download_storage, get_auto_download_storage_for, sanitize_filename,
        },
        get_auto_accept_config, get_auto_download_config, get_timestamp_secs,
    },
};
use futures::AsyncReadExt;
use libp2p::PeerId;
use libsignal_protocol::{DeviceId, ProtocolAddress};
use tokio::{fs::create_dir_all, sync::mpsc};
use zeroize::Zeroize;

pub async fn handle_incoming(
    from: PeerId,
    ciphertext: Vec<u8>,
    db: SharedDatabase,
    cache_dir: &std::path::Path,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let peer_id_str = from.to_base58();

    let encrypted_payload = match bincode::deserialize::<WireMessage>(&ciphertext) {
        Ok(WireMessage::ContactResponse(response)) => {
            let _ = handle_fc_response(response, db.clone(), cmd_tx, event_tx).await;
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

            // not the best way of fetching the contact by peer id
            let mut contact = Contact::load_all(&*db.clone().0.lock().await)?
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
        Ok(WireMessage::FileTransfer(chunk)) => {
            // not the best way of fetching the contact by peer id
            let contact = Contact::load_all(&*db.clone().0.lock().await)?
                .into_iter()
                .find(|c| c.peer_id == peer_id_str)
                .ok_or_else(|| KursalError::Identity("unknown peer".to_string()))?;

            if contact.blocked {
                return Ok(()); // ignore
            }

            let key = format!(
                "recv:{}:{}",
                hex::encode(contact.user_id.0),
                hex::encode(chunk.transfer_id)
            );

            let entry_bytes =
                db.0.lock()
                    .await
                    .raw_read(TABLE_FILE_TRANSFERS, &key)?
                    .ok_or(KursalError::Storage("Unknown transfer".to_string()))?;
            let mut entry = FileReceiveEntry::deserialize(&entry_bytes)?;

            let decrypted = stream_decrypt(&entry.key, &chunk.data)?;

            let offset = chunk.index as u64 * FILE_CHUNK_SIZE as u64;
            if offset + decrypted.len() as u64 > entry.file_size {
                return Err(KursalError::Storage("Chunk exceeds file size".to_string()));
            }

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(&entry.save_path)
                .map_err(KursalError::Io)?;
            std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(offset))
                .map_err(KursalError::Io)?;
            std::io::Write::write_all(&mut file, &decrypted).map_err(KursalError::Io)?;

            let idx = chunk.index as usize;
            entry.received_chunks[idx / 8] |= 1 << (idx % 8);
            db.0.lock()
                .await
                .raw_write(TABLE_FILE_TRANSFERS, &key, &entry.serialize()?)?;

            let chunk_count = usize::try_from(entry.file_size.div_ceil(FILE_CHUNK_SIZE as u64))
                .map_err(|err| KursalError::Storage(err.to_string()))?;
            let all_received = entry.received_chunks.iter().enumerate().all(|(i, byte)| {
                let bits_in_byte = (chunk_count - i * 8).min(8);
                let mask = if bits_in_byte >= 8 {
                    0xFFu8
                } else {
                    (1u8 << bits_in_byte) - 1
                };
                byte & mask == mask
            });

            let chunks_received = entry
                .received_chunks
                .iter()
                .map(|byte| byte.count_ones() as u64)
                .sum::<u64>();

            event_tx
                .send(AppEvent::FileTransferProgress {
                    transfer_id: MessageId(chunk.transfer_id),
                    bytes_transferred: (chunks_received * FILE_CHUNK_SIZE as u64)
                        .min(entry.file_size),
                    total_bytes: entry.file_size,
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            // TODO: check hash
            if all_received {
                db.0.lock().await.raw_delete(TABLE_FILE_TRANSFERS, &key)?;
                event_tx
                    .send(AppEvent::FileReceived {
                        contact_id: contact.user_id,
                        save_path: entry.save_path,
                    })
                    .await
                    .map_err(|err| KursalError::Network(err.to_string()))?;
            }

            return Ok(());
        }
        Ok(WireMessage::Encrypted(data)) => data,
        Err(_e) => {
            // log::debug!("Rejected invalid WireMessage from {}: {}", peer_id_str, e);
            return Ok(());
        }
    };

    // not the best way of fetching the contact by peer id + this is litteraly fetching another time...
    let mut contact = Contact::load_all(&*db.clone().0.lock().await)?
        .into_iter()
        .find(|c| c.peer_id == peer_id_str)
        .ok_or_else(|| KursalError::Identity("unknown peer".to_string()))?;

    if contact.blocked {
        return Ok(()); // ignore
    }

    let address = ProtocolAddress::new(hex::encode(contact.user_id.0), DeviceId::new(1u8).unwrap());
    let now = get_timestamp_secs()?;

    let received = message_receive(db.clone(), &address, &encrypted_payload).await?;
    let kmessage = KursalMessage::deserialize(&received)?;

    match kmessage {
        KursalMessage::Typing => {
            event_tx
                .send(AppEvent::TypingIndicator {
                    contact_id: contact.user_id,
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            return Ok(());
        }

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
            StoredMessage::delete(
                &*db.clone().0.lock().await,
                &contact.user_id,
                &del.target_id,
            )?;

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

        KursalMessage::ReactionAdd(ref r) => {
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

        KursalMessage::Text(_) => {
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

            send_delivery_receipt(db.clone(), &encrypted_payload, msg_id, &contact, cmd_tx).await?;
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

        KursalMessage::FileOffer(ref file) => {
            let filename = sanitize_filename(&file.filename);
            let offer_id = file.id;
            let size_bytes = file.size_bytes;
            let file_random = file.random;

            let stored = StoredMessage {
                id: file.id,
                contact_id: contact.user_id.clone(),
                payload: kmessage,
                timestamp: now,
                direction: Direction::Received,
                status: MessageStatus::Delivered,
                raw_ciphertext: None,
                edited: false,
                reactions: Vec::with_capacity(0),
            };

            stored.save(&*db.0.lock().await)?;

            let mut autodownload = None;
            let auto_accept = get_auto_accept_config(&*db.0.lock().await);

            if auto_accept.size_cap_bytes >= size_bytes
                && ((auto_accept.mode == "verified" && contact.verified)
                    || auto_accept.mode == "all")
            {
                let auto_config = get_auto_download_config(&*db.0.lock().await);

                let contact_hex = hex::encode(contact.user_id.0);

                let size = if auto_config.scope == "all_contacts" {
                    get_auto_download_storage(cache_dir.to_path_buf())
                } else {
                    get_auto_download_storage_for(cache_dir.to_path_buf(), contact_hex.clone())
                };

                match size {
                    Ok(size) => {
                        if size.saturating_add(size_bytes) <= auto_config.limit_bytes {
                            let root = cache_dir.join("files").join(contact_hex);
                            create_dir_all(&root).await.map_err(KursalError::Io)?;

                            let path =
                                root.join(format!("{}-{}", hex::encode(offer_id.0), filename));

                            autodownload = Some(path.to_string_lossy().into_owned());
                        }
                    }
                    Err(err) => {
                        log::error!("Could not get auto download folder size: {err}");
                    }
                }
            }

            let entry = FileIncomingEntry {
                their_random: file_random,
                file_size: size_bytes,
            };

            db.0.lock().await.raw_write(
                TABLE_FILE_TRANSFERS,
                &format!(
                    "recv:{}:{}",
                    hex::encode(contact.user_id.0),
                    hex::encode(offer_id.0)
                ),
                &entry.serialize()?,
            )?;

            event_tx
                .send(AppEvent::FileOffered {
                    contact_id: contact.user_id.clone(),
                    filename,
                    offer_id,
                    size_bytes,
                    autodownload,
                })
                .await
                .map_err(|err| KursalError::Network(err.to_string()))?;

            send_delivery_receipt(db.clone(), &encrypted_payload, offer_id, &contact, cmd_tx)
                .await?;
        }

        KursalMessage::FileAccept(file) => {
            let file_entry_bytes =
                db.0.lock()
                    .await
                    .raw_read(
                        TABLE_FILE_TRANSFERS,
                        &format!(
                            "send:{}:{}",
                            hex::encode(contact.user_id.0),
                            hex::encode(file.offer_id.0)
                        ),
                    )?
                    .ok_or(KursalError::Storage(
                        "Could not find file transfer (is it revoked?)".to_string(),
                    ))?;
            let mut file_entry = FileTransferEntry::deserialize(&file_entry_bytes)?;

            let now = get_timestamp_secs()?;
            file_entry.last_accessed_at = Some(now);
            db.0.lock().await.raw_write(
                TABLE_FILE_TRANSFERS,
                &format!(
                    "send:{}:{}",
                    hex::encode(contact.user_id.0),
                    hex::encode(file.offer_id.0)
                ),
                &file_entry.serialize()?,
            )?;

            let cmd_tx_clone = cmd_tx.clone();

            tokio::spawn(async move {
                if let Err(err) = send_file_chunks(
                    contact,
                    file.offer_id,
                    file_entry.path,
                    file_entry.my_random,
                    file.random,
                    cmd_tx_clone,
                )
                .await
                {
                    log::warn!("file transfer failed: {err:?}");
                }
            });
        }

        KursalMessage::CallSignal(_) => {
            // TODO: Handle
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

pub async fn handle_incoming_stream(
    peer_id: PeerId,
    mut stream: libp2p::Stream,
    event_tx: mpsc::Sender<NetworkEvent>,
) {
    loop {
        let mut len_bytes = [0u8; 4];
        if stream.read_exact(&mut len_bytes).await.is_err() {
            break;
        }
        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > MAX_MESSAGE_SIZE {
            log::warn!("oversized stream message from {peer_id}, closing");
            break;
        }

        let mut data = vec![0u8; len];
        if stream.read_exact(&mut data).await.is_err() {
            break;
        }

        let _ = event_tx
            .send(NetworkEvent::MessageReceived {
                from: peer_id,
                data,
            })
            .await;
    }
}

pub async fn send_delivery_receipt(
    db: SharedDatabase,
    encrypted_payload: &[u8],
    message_id: MessageId,
    contact: &Contact,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<()> {
    // send delivery receipt
    let mut secret_key =
        db.0.lock()
            .await
            .raw_read(TABLE_SETTINGS, "dilithium_secret")?
            .ok_or(KursalError::Storage("No dilithium-5 secret".to_string()))?;

    let signed = dilithium_sign(&secret_key, encrypted_payload)?;
    secret_key.zeroize();

    let receipt = KursalMessage::DeliveryReceipt(DeliveryReceipt {
        message_id,
        signature: signed,
    });

    send_message(receipt, contact, db, cmd_tx).await?;

    Ok(())
}
