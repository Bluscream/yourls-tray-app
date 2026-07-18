use crate::common::{AppState, log_debug, is_scroll_lock_active};
use crate::config;
use crate::api::fetch_history;
use crate::i18n;
use arboard::Clipboard;
use clipboard_master::{CallbackResult, ClipboardHandler, Master};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use url::Url;
use notify_rust::Notification;
use device_query::{DeviceQuery, DeviceState, Keycode};

#[cfg(target_os = "windows")]
pub struct ClipboardMonitor {
    pub state: Arc<Mutex<AppState>>,
    pub main_thread_id: u32,
}

#[cfg(target_os = "windows")]
impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        thread::sleep(Duration::from_millis(100));

        let state_clone = self.state.clone();
        let main_thread_id = self.main_thread_id;

        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(_) => return,
            };

            if let Ok(text) = clipboard.get_text() {
                process_clipboard_text(text, &state_clone, &mut clipboard, main_thread_id);
            }
        });

        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, _error: std::io::Error) -> CallbackResult {
        CallbackResult::Next
    }
}

#[cfg(target_os = "linux")]
pub fn get_linux_clipboard() -> Result<String, std::io::Error> {
    let output = std::process::Command::new("wl-paste")
        .output();
    
    match output {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        _ => {
            let xclip_out = std::process::Command::new("xclip")
                .args(&["-selection", "clipboard", "-o"])
                .output()?;
            if xclip_out.status.success() {
                Ok(String::from_utf8_lossy(&xclip_out.stdout).trim().to_string())
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to read clipboard"))
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub fn set_linux_clipboard(text: &str) -> Result<(), std::io::Error> {
    use std::io::Write;
    let child = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn();

    match child {
        Ok(mut c) => {
            if let Some(mut stdin) = c.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = c.wait();
            Ok(())
        }
        _ => {
            let mut xclip_child = std::process::Command::new("xclip")
                .args(&["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(mut stdin) = xclip_child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = xclip_child.wait();
            Ok(())
        }
    }
}

pub fn process_clipboard_text(
    text: String,
    state_clone: &Arc<Mutex<AppState>>,
    #[cfg(target_os = "windows")] clipboard: &mut Clipboard,
    #[cfg(target_os = "windows")] main_thread_id: u32,
) -> Option<String> {
    let text = text.trim().to_string();
    
    let parsed_url = match Url::parse(&text) {
        Ok(url) => url,
        Err(_) => return None,
    };
    if parsed_url.scheme().len() <= 1 || parsed_url.scheme() == "file" {
        return None;
    }

    let (enabled, last_undo_pair, last_attempted, config, bypass_undo_write) = {
        let s = state_clone.lock().unwrap();
        (
            s.enabled,
            s.last_undo_pair.clone(),
            s.last_attempted_long_url.clone(),
            s.config.clone(),
            s.bypass_undo_write,
        )
    };

    if bypass_undo_write {
        let mut s = state_clone.lock().unwrap();
        s.bypass_undo_write = false;
        log_debug("Skipping clipboard processing: undo bypass active.");
        return None;
    }

    let shift_pressed = config.bypass_shift_key && {
        let device_state = DeviceState::new();
        let keys = device_state.get_keys();
        keys.contains(&Keycode::LShift) || keys.contains(&Keycode::RShift)
    };
    let scroll_lock_active = config.bypass_scroll_lock && is_scroll_lock_active();

    if shift_pressed || scroll_lock_active {
        log_debug(&format!("Bypassing URL shortening. Shift pressed: {}, Scroll Lock active: {}", shift_pressed, scroll_lock_active));
        return None;
    }

    log_debug(&format!("Clipboard URL detected: {}", text));

    log_debug(&format!("Active status: enabled={}", enabled));
    if !enabled {
        log_debug("App is disabled.");
        return None;
    }

    if config.servers.is_empty() {
        log_debug("No configured servers available.");
        return None;
    }

    let mut primary_server: Option<config::ServerConfig> = None;
    if config.selected_server == "Random" {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let idx = (timestamp % config.servers.len() as u128) as usize;
        primary_server = Some(config.servers[idx].clone());
    } else {
        if let Some(srv) = config.servers.iter().find(|s| s.name == config.selected_server) {
            primary_server = Some(srv.clone());
        }
    }

    let primary = match primary_server {
        Some(p) => p,
        None => {
            log_debug(&format!("Selected server '{}' not found in configuration.", config.selected_server));
            return None;
        }
    };

    if primary.api_url.trim().is_empty() || primary.signature.trim().is_empty() {
        log_debug("Primary server configuration is incomplete.");
        return None;
    }

    if !config.blacklist_regex.trim().is_empty() {
        match regex::Regex::new(&config.blacklist_regex) {
            Ok(re) => {
                if re.is_match(&text) {
                    log_debug(&format!("Ignoring URL because it matches blacklist_regex: {}", config.blacklist_regex));
                    return None;
                }
            }
            Err(e) => {
                log_debug(&format!("Invalid blacklist_regex pattern: {:?}. Pattern: {}", e, config.blacklist_regex));
            }
        }
    }

    if let Some((_, ref short_w)) = last_undo_pair {
        if text == *short_w {
            return None;
        }
    }

    for server in &config.servers {
        if !server.base_url.is_empty() && text.starts_with(&server.base_url) {
            log_debug(&format!("Ignoring URL because it is already a shortened link from server '{}'.", server.name));
            return None;
        }
    }

    if config.bypass_double_copy {
        if let Some(ref last_att) = last_attempted {
            if text == *last_att {
                log_debug("URL copied twice consecutively. Bypassing shortening.");
                let mut s = state_clone.lock().unwrap();
                s.last_attempted_long_url = None;
                return None;
            }
        }
    }

    {
        let mut s = state_clone.lock().unwrap();
        s.last_attempted_long_url = Some(text.clone());
    }

    log_debug(&format!("Calling primary server '{}' API to shorten URL: {}", primary.name, text));
    let encoded_url: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
    let api_call_url = format!(
        "{}?signature={}&action=shorturl&url={}&format=simple",
        primary.api_url, primary.signature, encoded_url
    );

    let agent = crate::common::get_agent(config.ignore_ssl_errors);
    let response = match agent.get(&api_call_url).timeout(Duration::from_secs(3)).call() {
        Ok(res) => match res.into_string() {
            Ok(s) => s.trim().to_string(),
            Err(e) => {
                log_debug(&format!("API response read error: {:?}", e));
                return None;
            }
        },
        Err(e) => {
            log_debug(&format!("API request failed: {:?}", e));
            return None;
        }
    };

    log_debug(&format!("API returned shortened URL: {}", response));

    if !response.starts_with("http://") && !response.starts_with("https://") {
        log_debug("API response was not a valid URL.");
        return None;
    }

    let slug = response
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    if config.shorten_on_all && !slug.is_empty() {
        for server in &config.servers {
            if server.name == primary.name {
                continue;
            }
            if server.api_url.trim().is_empty() || server.signature.trim().is_empty() {
                continue;
            }
            log_debug(&format!("Propagating slug '{}' to server '{}'", slug, server.name));
            let secondary_api_url = format!(
                "{}?signature={}&action=shorturl&url={}&keyword={}&format=simple",
                server.api_url, server.signature, encoded_url, slug
            );
            match agent.get(&secondary_api_url).timeout(Duration::from_secs(3)).call() {
                Ok(res) => {
                    if let Ok(body) = res.into_string() {
                        log_debug(&format!("Secondary server '{}' response: {}", server.name, body.trim()));
                    }
                }
                Err(e) => {
                    log_debug(&format!("Secondary server '{}' failed to shorten: {:?}", server.name, e));
                }
            }
        }
    }

    let mut write_ok = false;
    #[cfg(target_os = "windows")]
    {
        for _ in 0..5 {
            if clipboard.set_text(response.clone()).is_ok() {
                write_ok = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    #[cfg(target_os = "linux")]
    {
        if set_linux_clipboard(&response).is_ok() {
            write_ok = true;
        }
    }

    if write_ok {
        log_debug("Wrote shortened URL back to clipboard.");
    } else {
        log_debug("Failed to write shortened URL to clipboard.");
        return None;
    }

    let locale = i18n::get_locale(&config.locale);
    let body_text = i18n::t(i18n::Key::OriginalShortened, &locale)
        .replace("{text}", &text)
        .replace("{response}", &response)
        .replacen("{}", &text, 1)
        .replacen("{}", &response, 1);

    let _ = Notification::new()
        .summary(i18n::t(i18n::Key::ClipboardLinkShortened, &locale))
        .body(&body_text)
        .show();

    let updated_history = fetch_history(&config);
    {
        let mut s = state_clone.lock().unwrap();
        s.last_undo_pair = Some((text.clone(), response.clone()));
        s.history = updated_history;
        s.needs_menu_rebuild = true;
    }

    #[cfg(target_os = "windows")]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            main_thread_id,
            windows_sys::Win32::UI::WindowsAndMessaging::WM_USER,
            0,
            0,
        );
    }

    Some(response)
}

#[cfg(target_os = "windows")]
pub fn spawn_clipboard_monitor(state: Arc<Mutex<AppState>>, main_thread_id: u32) {
    thread::spawn(move || {
        log_debug("Spawning Clipboard Monitor thread...");
        let monitor = ClipboardMonitor {
            state,
            main_thread_id,
        };
        let mut master = Master::new(monitor).expect("Failed to initialize clipboard listener");
        log_debug("Clipboard Master starting run loop");
        master.run().expect("Clipboard listener loop failed");
    });
}

#[cfg(target_os = "linux")]
pub fn spawn_linux_clipboard_poll(state_monitor: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        log_debug("Spawning Clipboard Polling thread...");
        let mut last_seen_text = String::new();

        if let Ok(text) = get_linux_clipboard() {
            last_seen_text = text;
            log_debug(&format!("Initial clipboard content: '{}'", last_seen_text));
        }

        loop {
            thread::sleep(Duration::from_millis(300));
            match get_linux_clipboard() {
                Ok(text) => {
                    if !text.is_empty() && text != last_seen_text {
                        log_debug(&format!("Clipboard content changed to: '{}'", text));
                        last_seen_text = text.clone();
                        if let Some(shortened) = process_clipboard_text(text, &state_monitor) {
                            last_seen_text = shortened;
                        }
                    }
                }
                Err(e) => {
                    log_debug(&format!("Clipboard poll error: {:?}", e));
                }
            }
        }
    });
}
