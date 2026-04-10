use kursal_core::{
    Result, api::state::AppState, first_contact::ltc::LtcPayload, storage::file::KursalFile,
};
use tauri::{AppHandle, Manager, async_runtime::block_on};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

pub trait FileLoader {
    fn load(&self, handle: &AppHandle) -> impl std::future::Future<Output = Result<()>>;
}

impl FileLoader for KursalFile {
    async fn load(&self, handle: &AppHandle) -> Result<()> {
        let state = handle.state::<AppState>();

        match self {
            KursalFile::LtcPayload(bytes) => {
                let result = LtcPayload::deserialize(bytes);

                let _result = match result {
                    Ok(payload) => {
                        payload
                            .import_ltc(state.db.clone(), &*state.network.lock().await)
                            .await
                    }
                    Err(e) => Err(e),
                }?;
            }
        }

        Ok(())
    }
}

pub fn open_files(app: &AppHandle, files: Vec<(String, String)>) {
    for (path, file_name) in files {
        if let Ok(content) = std::fs::read(&path)
            && let Ok(kursal_file) = KursalFile::deserialize(&content)
        {
            let do_open = app
                .dialog()
                .message(kursal_file.get_warning())
                .title(format!("Opening {file_name}"))
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Open".to_string(),
                    "Cancel".to_string(),
                ))
                .blocking_show();

            if do_open {
                let app = app.clone();

                std::thread::spawn(move || {
                    let local = tokio::task::LocalSet::new();

                    block_on(local.run_until(async move {
                        if let Err(err) = FileLoader::load(&kursal_file, &app).await {
                            log::error!("Failed to open file {file_name}: {err}");
                        }
                    }));
                });
            }
        }
    }
}
