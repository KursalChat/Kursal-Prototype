use kursal_core::{
    api::{AppEvent, ConnectionStatus},
    apiserver::CoreEventEmitter,
    dto::{ContactResponse, MessageResponse},
};
use std::{collections::HashMap, sync::Arc};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, broadcast, oneshot::Sender};

fn emitter<S: serde::Serialize + Clone>(
    handle: &AppHandle,
    api_handle: &broadcast::Sender<CoreEventEmitter>,
    event: &str,
    payload: S,
) {
    api_handle
        .send(CoreEventEmitter {
            event: event.to_string(),
            payload: serde_json::to_value(payload.clone()).unwrap_or(serde_json::Value::Null),
        })
        .ok();

    handle.emit(event, payload).ok();
}

pub async fn handle_core_event(
    event: AppEvent,
    handle: &AppHandle,
    api_handle: &broadcast::Sender<CoreEventEmitter>,
    pending_nearby_clone: &Arc<Mutex<HashMap<String, Sender<bool>>>>,
) {
    match event {
        AppEvent::BackendSignal { signal, payload } => {
            emitter(
                handle,
                api_handle,
                "backend_signal",
                serde_json::json!({
                    "signal": signal,
                    "payload": payload
                }),
            );
        }

        AppEvent::MessageReceived { message, .. } => {
            emitter(
                handle,
                api_handle,
                "message_received",
                MessageResponse::from(message),
            );
        }

        AppEvent::TypingIndicator { contact_id } => {
            emitter(
                handle,
                api_handle,
                "typing_indicator",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                }),
            );
        }

        AppEvent::MessageEdited {
            contact_id,
            message_id,
            new_content,
        } => {
            emitter(
                handle,
                api_handle,
                "message_edited",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "newContent": new_content
                }),
            );
        }

        AppEvent::MessageDeleted {
            contact_id,
            message_id,
        } => {
            emitter(
                handle,
                api_handle,
                "message_deleted",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                }),
            );
        }

        AppEvent::ReactionAdded {
            contact_id,
            message_id,
            emoji,
        } => {
            emitter(
                handle,
                api_handle,
                "reaction_added",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "emoji": emoji,
                }),
            );
        }

        AppEvent::ReactionRemoved {
            contact_id,
            message_id,
            emoji,
        } => {
            emitter(
                handle,
                api_handle,
                "reaction_removed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0),
                    "emoji": emoji,
                }),
            );
        }

        AppEvent::DeliveryConfirmed {
            message_id,
            contact_id,
        } => {
            emitter(
                handle,
                api_handle,
                "delivery_confirmed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "messageId": hex::encode(message_id.0)
                }),
            );
        }

        AppEvent::ContactAdded { contact } => {
            emitter(
                handle,
                api_handle,
                "contact_added",
                ContactResponse::from(contact),
            );
        }

        AppEvent::ContactUpdated { contact } => {
            emitter(
                handle,
                api_handle,
                "contact_updated",
                ContactResponse::from(contact),
            );
        }

        AppEvent::ConnectionChange { contact_id, status } => {
            emitter(
                handle,
                api_handle,
                "connection_changed",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "status": match status {
                        ConnectionStatus::Connecting => "connecting",
                        ConnectionStatus::Relay => "relay",
                        ConnectionStatus::HolePunch => "holepunch",
                        ConnectionStatus::Direct => "direct",
                        ConnectionStatus::Disconnected => "disconnected",
                    }
                }),
            );
        }

        AppEvent::NearbyRequest {
            peer_id,
            session_name,
            decision_tx,
        } => {
            pending_nearby_clone
                .lock()
                .await
                .insert(peer_id.clone(), decision_tx);

            emitter(
                handle,
                api_handle,
                "nearby_request",
                serde_json::json!({
                    "peerId": peer_id,
                    "sessionName": session_name
                }),
            );
        }

        AppEvent::ContactRemoved { contact_id } => {
            emitter(
                handle,
                api_handle,
                "contact_removed",
                serde_json::json!({
                    "peerId": hex::encode(contact_id.0),
                }),
            );
        }
        AppEvent::FileOffered {
            contact_id,
            offer_id,
            filename,
            size_bytes,
            autodownload,
        } => {
            emitter(
                handle,
                api_handle,
                "file_offered",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "offerId": hex::encode(offer_id.0),
                    "filename": filename,
                    "sizeBytes": size_bytes,
                    "autodownload": autodownload,
                }),
            );
        }
        AppEvent::FileTransferProgress {
            transfer_id,
            bytes_transferred,
            total_bytes,
        } => {
            emitter(
                handle,
                api_handle,
                "file_transfer_progress",
                serde_json::json!({
                    "transferId": hex::encode(transfer_id.0),
                    "bytesTransferred": bytes_transferred,
                    "totalBytes": total_bytes
                }),
            );
        }
        AppEvent::FileReceived {
            contact_id,
            save_path,
        } => {
            emitter(
                handle,
                api_handle,
                "file_received",
                serde_json::json!({
                    "contactId": hex::encode(contact_id.0),
                    "savePath": save_path,
                }),
            );
        }
    }
}
