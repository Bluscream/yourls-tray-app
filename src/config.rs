use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use url::Url;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub signature: String,
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
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            api_url: String::new(),
            signature: String::new(),
            enabled: true,
            blacklist_regex: String::new(),
            bypass_double_copy: true,
            bypass_shift_key: true,
            bypass_scroll_lock: true,
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

    // Inference logic
    // 1. If base_url is empty but api_url is set, infer base_url
    if config.base_url.trim().is_empty() && !config.api_url.trim().is_empty() {
        if let Ok(mut url) = Url::parse(&config.api_url) {
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
            config.base_url = url.to_string();
        }
    }
    // 2. If api_url is empty but base_url is set, infer api_url
    else if config.api_url.trim().is_empty() && !config.base_url.trim().is_empty() {
        if let Ok(mut url) = Url::parse(&config.base_url) {
            let current_path = url.path().trim_end_matches('/');
            url.set_path(&format!("{}/yourls-api.php", current_path));
            url.set_query(None);
            url.set_fragment(None);
            config.api_url = url.to_string();
        }
    }



    config
}

pub fn save_config(config: &Config) {
    let path = get_config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}
