pub mod account;
pub mod accounting;
pub mod analytics;
pub mod auditor;
pub mod export;
pub mod ingest;
pub mod locale;
pub mod parser;
pub mod pricing;
pub mod settings;
pub mod storage;

pub fn data_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Codex Unified Monitor")
}
pub fn codex_home() -> std::path::PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".codex"))
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn digest(value: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(value.as_ref()))
}
