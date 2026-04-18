use clap::{Parser, Subcommand};
use kursal_cli::CLIArgs;
use kursal_core::dto::{ContactResponse, MessageResponse};
use kursal_core::storage::api_server_enabled;
use kursal_core::{
    api::{AppEvent, ConnectionStatus, CoreCommand, state::AppState},
    identity::{
        self,
        keychain::{self, KeychainConfig},
    },
    network::{NetworkManager, dispatch_events},
};
use std::{collections::HashMap, sync::Arc};
use tauri::{AppHandle, Emitter, Manager, async_runtime::block_on};
use tokio::sync::{Mutex, mpsc, oneshot::Sender};

pub mod benchmark;
pub mod commands;
pub mod dirs;
pub mod error;
pub mod file;

#[derive(Parser, Default)]
#[command(version, about, long_about = None, author)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// ID of the used database
    #[arg(long)]
    database_id: Option<String>,
    /// [UNSAFE!!] Will write the database encryption key in a file - MEANT FOR DEBUGGING!
    #[arg(long)]
    unsafe_write_key_to_file: bool,
}

#[derive(Subcommand)]
enum Commands {
    Cli(CLIArgs),
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    match Args::try_parse() {
        Ok(args) => match args.command {
            Some(Commands::Cli(cli_args)) => {
                block_on(kursal_cli::run(
                    cli_args.config,
                    cli_args.validate,
                    cli_args.default_config,
                ));

                return;
            }
            _ => {}
        },
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                e.exit()
            }
            _ => {}
        },
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }

    builder
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            dirs::init_dirs(app).unwrap();
            let log_path = dirs::logs_dir().unwrap().join("kursal.log");

            let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
            kursal_core::logging::init_logging(&log_level, Some(&log_path.to_string_lossy()))
                .expect("failed to init logger");

            log::info!("Logging enabled — writing to {}", log_path.display());

            if let Err(ohno) = keychain::init_keychain() {
                log::error!(
                    "Could not initiate keychain. This may trigger a crash later on. Error: {ohno}"
                );
            } else {
                log::info!("Keychain initialized");
            }

            let app_data_dir = dirs::app_data_dir().unwrap();
            log::info!("Directories initialized");

            let args = Args::try_parse().unwrap_or_default();
            let db_path = app_data_dir.join(format!(
                "{}.db",
                args.database_id.clone().unwrap_or("storage".to_string())
            ));

            let keychain_config = KeychainConfig {
                storage_id: args.database_id.unwrap_or("master".to_string()),
                unsafe_write_key_to_file: args.unsafe_write_key_to_file,
            };

            log::info!(
                "About to init identity (db_path={}, storage_id={})",
                db_path.display(),
                keychain_config.storage_id
            );

            let db = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                block_on(identity::init(&db_path, &keychain_config, app_data_dir))
            })) {
                Ok(Ok(db)) => {
                    log::info!("Identity init succeeded");
                    db
                }
                Ok(Err(e)) => {
                    log::error!("Identity init returned error: {e}");
                    return Err(Box::new(e).into());
                }
                Err(panic_info) => {
                    let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown panic".to_string()
                    };
                    log::error!("Identity init PANICKED: {msg}");
                    return Err(format!("identity::init panicked: {msg}").into());
                }
            };
            let (network, event_rx, bt_event_rx) =
                block_on(NetworkManager::new(&db.0.blocking_lock())).unwrap();

            let (core_cmd_tx, core_cmd_rx) = mpsc::channel::<CoreCommand>(32);
            let (app_event_tx, mut app_event_rx) = mpsc::channel::<AppEvent>(16);
            let network_arc = Arc::new(Mutex::new(network));
            let pending_nearby = Arc::new(Mutex::new(HashMap::new()));

            let db_clone = db.clone();
            let network_clone = network_arc.clone();
            let app_tx_clone = app_event_tx.clone();
            let pending_nearby_clone = pending_nearby.clone();

            tauri::async_runtime::spawn(NetworkManager::spawn_rendezvous_publisher(
                db.clone(),
                network_arc.clone(),
            ));

            tauri::async_runtime::spawn(NetworkManager::spawn_rotation_scheduler(
                db.clone(),
                core_cmd_tx.clone(),
                network_arc.clone(),
            ));

            std::thread::spawn(move || {
                let local = tokio::task::LocalSet::new();

                block_on(local.run_until(dispatch_events(
                    event_rx,
                    bt_event_rx,
                    core_cmd_rx,
                    db_clone,
                    network_clone,
                    app_tx_clone,
                )));
            });

            let handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                while let Some(event) = app_event_rx.recv().await {
                    handle_core_event(event, &handle, &pending_nearby_clone).await;
                }
            });

            app.manage(AppState {
                db: db.clone(),
                network: network_arc.clone(),
                app_event_tx: app_event_tx.clone(),
                core_cmd_tx: core_cmd_tx.clone(),
                pending_nearby: pending_nearby.clone(),
            });

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use std::time::Duration;

                    let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60));

                    loop {
                        interval.tick().await;
                        let _ = check_for_updates_impl(handle.clone(), false).await;
                    }
                });
            }

            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                use crate::file::open_files;

                let files: Vec<(String, String)> = std::env::args()
                    .skip(1)
                    .filter(|a| !a.starts_with('-'))
                    .filter_map(|path| {
                        let p = std::path::PathBuf::from(&path);
                        let name = p.file_name()?.to_string_lossy().to_string();
                        Some((path, name))
                    })
                    .collect();

                if !files.is_empty() {
                    open_files(&app.handle(), files);
                }
            }

            // spawn local API server if enabled
            // TODO: kursal_core::storage::set_api_server_enabled(&*db.0.lock().await, false).unwrap();
            let core_cmd_tx_clone = core_cmd_tx.clone();
            let db_clone = db.clone();
            let network_clone = network_arc.clone();
            let pending_nearby_clone = pending_nearby.clone();
            tauri::async_runtime::spawn(async move {
                let is_api_server_enabled = api_server_enabled(&*db.0.lock().await);

                match is_api_server_enabled {
                    Ok(true) => {
                        log::info!("Starting API server...");

                        // TODO: proper password!!!
                        let auth_token = "root".to_string();

                        if let Err(err) = kursal_core::apiserver::run_server(
                            auth_token,
                            core_cmd_tx_clone,
                            db_clone,
                            network_clone,
                            pending_nearby_clone,
                        )
                        .await
                        {
                            log::error!("Error while running API server: {err}");
                        }
                    }
                    Ok(false) => {
                        log::info!("API server not enabled");
                    }
                    Err(err) => {
                        log::error!("Could not start API server: {err}");
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::generate_otp,
            commands::publish_otp,
            commands::fetch_otp,
            commands::export_ltc,
            commands::import_ltc,
            commands::start_nearby,
            commands::stop_nearby,
            commands::get_nearby_peers,
            commands::connect_nearby,
            commands::accept_nearby,
            commands::decline_nearby,
            commands::get_contacts,
            commands::remove_contact,
            commands::send_text,
            commands::send_typing_indicator,
            commands::get_messages,
            commands::delete_local_message,
            commands::get_security_code,
            commands::confirm_security_code,
            commands::set_contact_blocked,
            commands::rotate_peer_id,
            commands::get_local_peer_id,
            commands::get_local_user_id_hex,
            commands::set_relay_server_enabled,
            commands::get_local_user_profile,
            commands::broadcast_profile,
            commands::share_profile,
            commands::check_for_updates,
            commands::open_log_folder,
            commands::delete_message_for_everyone,
            commands::edit_message,
            commands::add_reaction,
            commands::remove_reaction,
            commands::accept_file_offer,
            commands::send_file_offer,
            //
            benchmark::run_otp_benchmark,
            benchmark::cancel_benchmark,
            benchmark::is_benchmark_running,
        ])
        .build(tauri::generate_context!())
        .expect("error while building application")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                use crate::file::open_files;

                let files: Vec<(String, String)> = urls
                    .iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .filter_map(|p| {
                        let name = p.file_name()?.to_string_lossy().to_string();
                        let path = p.to_string_lossy().to_string();
                        Some((path, name))
                    })
                    .collect();

                if !files.is_empty() {
                    open_files(_app, files);
                }
            }
        });
}

async fn handle_core_event(
    event: AppEvent,
    handle: &AppHandle,
    pending_nearby_clone: &Arc<Mutex<HashMap<String, Sender<bool>>>>,
) {
    match event {
        AppEvent::MessageReceived { message, .. } => {
            handle
                .emit("message_received", MessageResponse::from(message))
                .ok();
        }

        AppEvent::TypingIndicator { contact_id } => {
            handle
                .emit(
                    "typing_indicator",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                    }),
                )
                .ok();
        }

        AppEvent::MessageEdited {
            contact_id,
            message_id,
            new_content,
        } => {
            handle
                .emit(
                    "message_edited",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "messageId": hex::encode(message_id.0),
                        "newContent": new_content
                    }),
                )
                .ok();
        }

        AppEvent::MessageDeleted {
            contact_id,
            message_id,
        } => {
            handle
                .emit(
                    "message_deleted",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "messageId": hex::encode(message_id.0),
                    }),
                )
                .ok();
        }

        AppEvent::ReactionAdded {
            contact_id,
            message_id,
            emoji,
        } => {
            handle
                .emit(
                    "reaction_added",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "messageId": hex::encode(message_id.0),
                        "emoji": emoji,
                    }),
                )
                .ok();
        }

        AppEvent::ReactionRemoved {
            contact_id,
            message_id,
            emoji,
        } => {
            handle
                .emit(
                    "reaction_removed",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "messageId": hex::encode(message_id.0),
                        "emoji": emoji,
                    }),
                )
                .ok();
        }

        AppEvent::DeliveryConfirmed {
            message_id,
            contact_id,
        } => {
            handle
                .emit(
                    "delivery_confirmed",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "messageId": hex::encode(message_id.0)
                    }),
                )
                .ok();
        }

        AppEvent::ContactAdded { contact } => {
            handle
                .emit("contact_added", ContactResponse::from(contact))
                .ok();
        }

        AppEvent::ContactUpdated { contact } => {
            handle
                .emit("contact_updated", ContactResponse::from(contact))
                .ok();
        }

        AppEvent::PeerIdRotated { new_addresses } => {
            handle.emit("peer_id_rotated", new_addresses).ok();
        }

        AppEvent::ConnectionChange { contact_id, status } => {
            handle
                .emit(
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
                )
                .ok();
        }

        AppEvent::LTCExpiringSoon { hours_remaining } => {
            handle.emit("ltc_expiring_soon", hours_remaining).ok();
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
            handle
                .emit(
                    "nearby_request",
                    serde_json::json!({
                        "peerId": peer_id,
                        "sessionName": session_name
                    }),
                )
                .ok();
        }

        AppEvent::ContactRemoved { contact_id } => {
            handle
                .emit("contact_removed", hex::encode(contact_id.0))
                .ok();
        }
        AppEvent::FileOffered {
            contact_id,
            offer_id,
            filename,
            size_bytes,
        } => {
            handle
                .emit(
                    "file_offered",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "offerId": hex::encode(offer_id.0),
                        "filename": filename,
                        "sizeBytes": size_bytes
                    }),
                )
                .ok();
        }
        AppEvent::FileTransferProgress {
            transfer_id,
            bytes_transferred,
            total_bytes,
        } => {
            handle
                .emit(
                    "file_transfer_progress",
                    serde_json::json!({
                        "transferId": hex::encode(transfer_id.0),
                        "bytesTransferred": bytes_transferred,
                        "totalBytes": total_bytes
                    }),
                )
                .ok();
        }
        AppEvent::FileReceived {
            contact_id,
            save_path,
        } => {
            handle
                .emit(
                    "file_received",
                    serde_json::json!({
                        "contactId": hex::encode(contact_id.0),
                        "savePath": save_path,
                    }),
                )
                .ok();
        }
    }
}

#[cfg(all(not(any(target_os = "android", target_os = "ios")), dev))]
pub(crate) async fn check_for_updates_impl(
    _app: AppHandle,
    _manual: bool,
) -> tauri_plugin_updater::Result<()> {
    Ok(()) // do not try to update on dev mode
}

#[cfg(not(any(target_os = "android", target_os = "ios", dev)))]
pub(crate) async fn check_for_updates_impl(
    app: AppHandle,
    manual: bool,
) -> tauri_plugin_updater::Result<()> {
    log::debug!("checking for updates... manual={manual}");
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    use tauri_plugin_updater::UpdaterExt;

    if let Some(update) = app.updater()?.check().await? {
        let do_update = app
            .dialog()
            .message(format!(
                "Version {} is available! (you are on {}){}",
                update.version,
                update.current_version,
                update
                    .body
                    .as_ref()
                    .map(|v| format!("\n\nRelease notes: {v}"))
                    .unwrap_or("".to_string())
            ))
            .title("Update Available")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Update now".to_string(),
                "Later".to_string(),
            ))
            .blocking_show();

        if !do_update {
            return Ok(());
        }

        let mut downloaded = 0;

        update
            .download_and_install(
                |chunk_len, content_len| {
                    downloaded += chunk_len;
                    log::debug!("[updater] downloaded {downloaded} out of {content_len:?}");
                },
                || {
                    log::debug!("[updater] download finished");
                },
            )
            .await?;

        log::info!("[updater] update installed");

        let do_restart = app
            .dialog()
            .message(
                "Would you like to restart Kursal to apply the update? You can also restart later.",
            )
            .title("Update Installed")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Restart now".to_string(),
                "Later".to_string(),
            ))
            .blocking_show();

        if do_restart {
            app.restart();
        }
    } else if manual {
        app.dialog()
            .message("You're all set! Kursal is currently running the newest version available.")
            .title("No Updates Available")
            .kind(MessageDialogKind::Info)
            .blocking_show();
    }

    Ok(())
}
