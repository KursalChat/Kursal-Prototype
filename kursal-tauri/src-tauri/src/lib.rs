use crate::core_event::handle_core_event;
use crate::deep_link::map_deep_links;
use clap::{Parser, Subcommand};
use kursal_cli::CLIArgs;
use kursal_core::apiserver::CoreEventEmitter;
use kursal_core::storage::{api_server_config, api_server_password, should_reset_full_app};
use kursal_core::{
    api::{AppEvent, CoreCommand, state::AppState},
    identity::{
        self,
        keychain::{self, KeychainConfig},
    },
    network::{NetworkManager, dispatch_events},
};
use std::fs::remove_dir_all;
use std::{collections::HashMap, sync::Arc};
use tauri::{Manager, async_runtime::block_on};
use tauri_plugin_deep_link::DeepLinkExt;
use tokio::sync::{Mutex, broadcast, mpsc};

pub mod benchmark;
pub mod commands;
pub mod core_event;
pub mod deep_link;
pub mod dirs;
pub mod error;
pub mod file;
pub mod window_menu;

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
        Ok(args) => {
            if let Some(Commands::Cli(cli_args)) = args.command {
                block_on(kursal_cli::run(
                    cli_args.config,
                    cli_args.validate,
                    cli_args.default_config,
                ));

                return;
            }
        }
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                e.exit()
            }
            _ => {}
        },
    }

    #[cfg(dev)]
    let mut builder = tauri::Builder::default();

    #[cfg(not(dev))]
    let mut builder = tauri::Builder::default();

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .on_window_event(|window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    window.hide().unwrap();
                }
            });
    }

    #[cfg(not(any(target_os = "android", target_os = "ios", dev)))]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_autostart::Builder::new().build())
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                let _ = app
                    .get_webview_window("main")
                    .expect("no main window")
                    .set_focus();
            }));
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }

    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            window_menu::setup(app).unwrap();

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

            // check for reset flags
            let db_clone = db.clone();
            let should_reset = block_on(async move {
                let db_lock = db_clone.0.lock().await;
                should_reset_full_app(&db_lock)
            });
            if should_reset {
                if let Ok(cache_dir) = dirs::cache_dir() {
                    let _ = remove_dir_all(cache_dir);
                }
                if let Ok(logs_dir) = dirs::logs_dir() {
                    let _ = remove_dir_all(logs_dir);
                }
                if let Ok(app_data_dir) = dirs::app_data_dir() {
                    let _ = remove_dir_all(app_data_dir);
                }

                app.handle().restart();
            }


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

            let cache_dir = dirs::cache_dir()?;
            std::thread::spawn(move || {
                let local = tokio::task::LocalSet::new();

                block_on(local.run_until(dispatch_events(
                    event_rx,
                    bt_event_rx,
                    core_cmd_rx,
                    db_clone,
                    network_clone,
                    app_tx_clone,
                    cache_dir,
                )));
            });

            let handle = app.handle().clone();
            let (api_server_tx, _): (broadcast::Sender<CoreEventEmitter>, _) = broadcast::channel(64);

            let api_server_tx_clone = api_server_tx.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(event) = app_event_rx.recv().await {
                    handle_core_event(event, &handle, &api_server_tx_clone, &pending_nearby_clone).await;
                }
            });

            let start_urls = app.deep_link().get_current()?.map(map_deep_links);

            app.manage(AppState {
                db: db.clone(),
                network: network_arc.clone(),
                app_event_tx: app_event_tx.clone(),
                core_cmd_tx: core_cmd_tx.clone(),
                pending_nearby: pending_nearby.clone(),
                db_path,
                deep_links: Mutex::new(start_urls),
                keychain_config
            });

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let handle = app.handle().clone();
                let db_clone = db.clone();
                tauri::async_runtime::spawn(async move {
                    use std::time::Duration;

                    let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60));

                    loop {
                        use kursal_core::storage::get_updater_enabled;

                        interval.tick().await;

                        let enabled = get_updater_enabled(&*db_clone.0.lock().await).unwrap_or(true);

                        if enabled {
                            let _ = check_for_updates_impl(handle.clone(), false).await;
                        }
                    }
                });
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                if let Err(err) = deep_link::deep_link_handler(&handle, map_deep_links(event.urls())) {
                    log::error!("Error while handling deep linking: {err}");
                }
            });

            // spawn local API server if enabled
            let core_cmd_tx_clone = core_cmd_tx.clone();
            let db_clone = db.clone();
            let network_clone = network_arc.clone();
            let pending_nearby_clone = pending_nearby.clone();
            tauri::async_runtime::spawn(async move {
                log::info!("Starting API server...");

                if let Ok(api_config) = api_server_config(&*db.0.lock().await)
                && api_config.enabled
                && let Ok(api_token) = api_server_password(&*db.0.lock().await) {
                    if let Err(err) = kursal_core::apiserver::run_server(
                        api_token,
                        api_config,
                        core_cmd_tx_clone,
                        db_clone,
                        network_clone,
                        pending_nearby_clone,
                        api_server_tx,
                    )
                    .await
                    {
                        log::error!("Error while running API server: {err}");
                    }
                } else {
                    log::info!("API server could not start because of an invalid auth_token setting. Please re-generate the token.");
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
            commands::list_blocked_contacts,
            commands::rotate_peer_id,
            commands::get_local_peer_id,
            commands::get_local_user_id_hex,
            commands::get_local_user_profile,
            commands::broadcast_profile,
            commands::share_profile,
            commands::check_for_updates,
            commands::open_log_folder,
            commands::open_files_folder,
            commands::delete_message_for_everyone,
            commands::edit_message,
            commands::add_reaction,
            commands::remove_reaction,
            commands::accept_file_offer,
            commands::send_file_offer,
            //
            commands::export_backup,
            commands::import_backup,
            commands::get_updater_enabled,
            commands::set_updater_enabled,
            commands::get_storage_usage,
            commands::get_auto_download_config,
            commands::set_auto_download_config,
            commands::get_auto_accept_config,
            commands::set_auto_accept_config,
            commands::list_shared_files,
            commands::revoke_shared_file,
            commands::revoke_shared_files_bulk,
            commands::get_nearby_share_enabled,
            commands::set_nearby_share_enabled,
            commands::get_relay_config,
            commands::set_relay_config,
            commands::get_listening_port,
            commands::set_listening_port,
            commands::get_local_api_config,
            commands::set_local_api_config,
            commands::generate_local_api_token,
            commands::delete_all_local_data,
            commands::clear_message_history,
            commands::get_peer_rotation_interval,
            commands::set_peer_rotation_interval,
            commands::get_typing_indicators_enabled,
            commands::set_typing_indicators_enabled,
            commands::frontend_ready,
            //
            benchmark::run_otp_benchmark,
            benchmark::cancel_benchmark,
            benchmark::is_benchmark_running,
        ])
        .build(tauri::generate_context!())
        .expect("error while building application")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            {
                if let tauri::RunEvent::Reopen { .. } = _event
                    && let Some(win) = _app.get_webview_window("main") {
                        win.show().unwrap();
                        win.set_focus().unwrap();
                }

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
            }
        });
}

#[cfg(all(not(any(target_os = "android", target_os = "ios")), dev))]
pub(crate) async fn check_for_updates_impl(
    _app: tauri::AppHandle,
    _manual: bool,
) -> tauri_plugin_updater::Result<()> {
    Ok(()) // do not try to update on dev mode
}

#[cfg(not(any(target_os = "android", target_os = "ios", dev)))]
pub(crate) async fn check_for_updates_impl(
    app: tauri::AppHandle,
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
