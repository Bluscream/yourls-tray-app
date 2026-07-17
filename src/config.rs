use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use url::Url;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub signature: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {


    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub blacklist_regex: String,
    #[serde(default = "default_true")]
    pub bypass_double_copy: bool,
    #[serde(default = "default_true")]
    pub bypass_shift_key: bool,
    #[serde(default = "default_true")]
    pub bypass_scroll_lock: bool,
    #[serde(default = "default_true")]
    pub enable_undo: bool,

    // New multi-server fields
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
    #[serde(default = "default_selected_server")]
    pub selected_server: String,
    #[serde(default)]
    pub shorten_on_all: bool,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_log_file_name")]
    pub log_file_name: String,
    #[serde(default)]
    pub check_update_on_startup: bool,
}

fn default_true() -> bool {
    true
}

fn default_selected_server() -> String {
    "Random".to_string()
}

fn default_locale() -> String {
    "auto".to_string()
}

fn default_log_file_name() -> String {
    "yourls-tray-app.log".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {

            enabled: true,
            blacklist_regex: String::new(),
            bypass_double_copy: true,
            bypass_shift_key: true,
            bypass_scroll_lock: true,
            enable_undo: true,
            servers: Vec::new(),
            selected_server: "Random".to_string(),
            shorten_on_all: false,
            locale: "auto".to_string(),
            log_file_name: "yourls-tray-app.log".to_string(),
            check_update_on_startup: false,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop();
        let local_config = exe_path.join("config.toml");
        if local_config.exists() {
            return local_config;
        }
    }
    
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    
    let mut home_path = PathBuf::from(home);
    home_path.push(".yourls-clipboard-shortener");
    let _ = fs::create_dir_all(&home_path);
    home_path.join("config.toml")
}

pub fn load_config() -> Config {
    let path = get_config_path();
    if !path.exists() {
        let default_config = Config::default();
        save_config(&default_config);
        return default_config;
    }

    let mut config = if let Ok(content) = fs::read_to_string(&path) {
        match toml::from_str(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                let mut log_path = std::env::temp_dir();
                log_path.push("yourls-tray-app.log");
                if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_path) {
                    use std::io::Write;
                    let _ = writeln!(f, "Failed to parse config.toml: {:?}", e);
                }
                Config::default()
            }
        }
    } else {
        Config::default()
    };



    // Run inference for each server
    for server in &mut config.servers {
        // 1. If base_url is empty but api_url is set, infer base_url
        if server.base_url.trim().is_empty() && !server.api_url.trim().is_empty() {
            if let Ok(mut url) = Url::parse(&server.api_url) {
                let mut path_segments: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
                if !path_segments.is_empty() && path_segments.last() == Some(&"yourls-api.php") {
                    path_segments.pop();
                }
                let new_path = path_segments.join("/");
                if new_path.is_empty() {
                    url.set_path("/");
                } else {
                    url.set_path(&format!("{}/", new_path));
                }
                url.set_query(None);
                url.set_fragment(None);
                server.base_url = url.to_string();
            }
        }
        // 2. If api_url is empty but base_url is set, infer api_url
        else if server.api_url.trim().is_empty() && !server.base_url.trim().is_empty() {
            if let Ok(mut url) = Url::parse(&server.base_url) {
                let current_path = url.path().trim_end_matches('/');
                url.set_path(&format!("{}/yourls-api.php", current_path));
                url.set_query(None);
                url.set_fragment(None);
                server.api_url = url.to_string();
            }
        }

        // 3. If server name is missing, extract domain from base_url or api_url
        if server.name.trim().is_empty() {
            let target_url = if !server.base_url.trim().is_empty() {
                &server.base_url
            } else {
                &server.api_url
            };
            if let Ok(url) = Url::parse(target_url) {
                if let Some(domain) = url.domain() {
                    server.name = domain.to_string();
                } else {
                    server.name = "Unknown".to_string();
                }
            } else {
                server.name = "Unknown".to_string();
            }
        }
    }

    crate::common::set_log_file_name(config.log_file_name.clone());

    config
}

pub fn save_config(config: &Config) {
    let path = get_config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}
