use crate::config;

pub struct AppState {
    pub enabled: bool,
    pub config: config::Config,
    pub last_attempted_long_url: Option<String>,
    pub last_undo_pair: Option<(String, String)>, // (long_url, short_url)
    pub history: Vec<(String, String)>,
    pub needs_menu_rebuild: bool,
    pub bypass_undo_write: bool,
}

pub fn log_debug(msg: &str) {
    let mut log_path = std::env::temp_dir();
    log_path.push("yourls-tray-app.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        use std::io::Write;
        let _ = writeln!(f, "{}", msg);
    }
}

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
                if let Some(name) = entry.file_name().to_str() {
                    if name.contains("scrolllock") {
                        let brightness_path = entry.path().join("brightness");
                        if let Ok(content) = std::fs::read_to_string(brightness_path) {
                            if content.trim() != "0" {
                                return true;
                            }
                        }
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
