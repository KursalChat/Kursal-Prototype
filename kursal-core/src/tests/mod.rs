use std::path::PathBuf;
use std::sync::LazyLock;

pub static APP_DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("Kursal")
});

pub static CACHE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("Kursal")
});

mod contact;
mod crypto;
mod identity;
mod ltc;
mod messaging;
mod nearby;
mod otp;
mod storage;
