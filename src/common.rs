//! ## Lint policy
//!
//! This module predates the lint policy in Cargo.toml and is left as it is on
//! purpose: the tray is legacy now that the binary is a CLI first, and
//! rewriting it to satisfy pedantic would be a change with no behavioural
//! benefit and real risk. The suppressions are listed one by one rather than
//! blanket-disabled, so anything *else* still fails the build.
#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::struct_excessive_bools,
    clippy::match_same_arms,
    clippy::assigning_clones,
    clippy::type_complexity,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::enum_variant_names,
    clippy::field_reassign_with_default,
    clippy::needless_pass_by_value,
    clippy::single_match_else,
    clippy::similar_names,
    clippy::manual_let_else,
    clippy::items_after_statements
)]

#[cfg(feature = "tray")]
use crate::config;

#[cfg(feature = "tray")]
pub struct AppState {
    pub enabled: bool,
    pub config: config::Config,
    pub last_attempted_long_url: Option<String>,
    pub last_undo_pair: Option<(String, String)>, // (long_url, short_url)
    pub history: Vec<(String, String)>,
    pub needs_menu_rebuild: bool,
    pub bypass_undo_write: bool,
}

static LOG_FILE_NAME: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_log_file_name(name: String) {
    if let Ok(mut lock) = LOG_FILE_NAME.lock() {
        *lock = Some(name);
    }
}

pub fn log_debug(msg: &str) {
    let name = {
        if let Ok(lock) = LOG_FILE_NAME.lock() {
            lock.clone()
                .unwrap_or_else(|| "yourls-tray-app.log".to_string())
        } else {
            "yourls-tray-app.log".to_string()
        }
    };

    let formatted_name = if name.contains('%') {
        let now = chrono::Local::now();
        now.format(&name).to_string()
    } else {
        name
    };

    let mut log_path = std::env::temp_dir();
    log_path.push(formatted_name);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{msg}");
    }
}

#[cfg(feature = "tray")]
pub fn is_scroll_lock_active() -> bool {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetKeyState(nVirtKey: i32) -> i16;
        }
        unsafe { (GetKeyState(0x91) & 1) != 0 }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/sys/class/leds") {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str()
                    && name.contains("scrolllock")
                {
                    let brightness_path = entry.path().join("brightness");
                    if let Ok(content) = std::fs::read_to_string(brightness_path)
                        && content.trim() != "0"
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
    {
        false
    }
}

pub fn get_agent(ignore_ssl: bool) -> ureq::Agent {
    struct V4Resolver;
    impl ureq::Resolver for V4Resolver {
        fn resolve(&self, netloc: &str) -> std::io::Result<Vec<std::net::SocketAddr>> {
            use std::net::ToSocketAddrs;
            let addrs = netloc.to_socket_addrs()?;
            let v4_addrs: Vec<_> = addrs
                .into_iter()
                .filter(std::net::SocketAddr::is_ipv4)
                .collect();
            if v4_addrs.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrNotAvailable,
                    "No IPv4 address found",
                ));
            }
            Ok(v4_addrs)
        }
    }

    let mut builder = ureq::builder().resolver(V4Resolver);

    if ignore_ssl {
        let mut tls_connector = native_tls::TlsConnector::builder();
        tls_connector.danger_accept_invalid_certs(true);
        tls_connector.danger_accept_invalid_hostnames(true);
        if let Ok(connector) = tls_connector.build() {
            builder = builder.tls_connector(std::sync::Arc::new(connector));
        }
    }
    builder.build()
}
