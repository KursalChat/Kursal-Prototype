use kursal_core::api::{
    AppEvent,
    state::{AppState, DeepLink},
};
use tauri::{Manager, async_runtime::block_on};

pub fn deep_link_handler(handle: &tauri::AppHandle, urls: Vec<DeepLink>) -> tauri::Result<()> {
    log::info!("Opening deep link URLs: {:?}", urls);

    let state = handle.state::<AppState>();

    for url in urls {
        match url.category.as_deref() {
            Some("settings") => {
                let state = state.clone();
                block_on(async move {
                    state
                        .app_event_tx
                        .send(AppEvent::BackendSignal {
                            signal: "open_settings".to_string(),
                            payload: String::with_capacity(0),
                        })
                        .await
                        .ok();
                });

                continue;
            }
            Some("otp") => {
                let state = state.clone();
                block_on(async move {
                    state
                        .app_event_tx
                        .send(AppEvent::BackendSignal {
                            signal: "open_otp".to_string(),
                            payload: url.path,
                        })
                        .await
                        .ok();
                });

                continue;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn map_deep_links(urls: Vec<tauri::Url>) -> Vec<DeepLink> {
    urls.into_iter()
        .map(|url| {
            let category = url.host_str().map(str::to_owned);
            let path = url.path()[1..].to_owned();
            DeepLink { category, path }
        })
        .collect::<Vec<_>>()
}
