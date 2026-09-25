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

use crate::api::fetch_history;
use crate::common::{AppState, is_scroll_lock_active, log_debug};
use crate::i18n;
use device_query::{DeviceQuery, DeviceState, Keycode};
use notify_rust::Notification;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use url::Url;

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
    let output = std::process::Command::new("wl-paste").output();

    match output {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        _ => {
            let xclip_out = std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()?;
            if xclip_out.status.success() {
                Ok(String::from_utf8_lossy(&xclip_out.stdout)
                    .trim()
                    .to_string())
            } else {
                Err(std::io::Error::other("Failed to read clipboard"))
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

    if let Ok(mut c) = child {
        if let Some(mut stdin) = c.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = c.wait();
        Ok(())
    } else {
        let mut xclip_child = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = xclip_child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = xclip_child.wait();
        Ok(())
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

    let (enabled, _last_undo_pair, _last_attempted, config, bypass_undo_write) = {
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
        log_debug(&format!(
            "Bypassing URL shortening. Shift pressed: {shift_pressed}, Scroll Lock active: {scroll_lock_active}"
        ));
        return None;
    }

    log_debug(&format!("Clipboard URL detected: {text}"));

    log_debug(&format!("Active status: enabled={enabled}"));
    if !enabled {
        log_debug("App is disabled.");
        return None;
    }

    // A URL the user asked never to shorten is left in the clipboard exactly
    // as it was found.
    match crate::shorten::is_blacklisted(&parsed_url, &config) {
        Ok(true) => {
            log_debug(&format!("{text} matches blacklist_regex; leaving it alone"));
            return None;
        }
        Ok(false) => {}
        Err(e) => {
            log_debug(&format!("{e}; refusing to shorten anything"));
            return None;
        }
    }

    {
        let mut s = state_clone.lock().unwrap();
        s.last_attempted_long_url = Some(text.clone());
    }

    // The same code the command line uses. Two copies of "call the API and
    // read the answer" would drift, and only one of them would be exercised.
    // No override: the tray picks its server from the menu, which is what
    // `selected_server` in the config already holds.
    let response = match crate::shorten::shorten(&parsed_url, &config, None) {
        Ok(short) => short,
        Err(e) => {
            log_debug(&format!("could not shorten {text}: {e}"));
            return None;
        }
    };
    log_debug(&format!("API returned shortened URL: {response}"));

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
            log_debug(&format!("Initial clipboard content: '{last_seen_text}'"));
        }

        loop {
            thread::sleep(Duration::from_millis(300));
            match get_linux_clipboard() {
                Ok(text) => {
                    if !text.is_empty() && text != last_seen_text {
                        log_debug(&format!("Clipboard content changed to: '{text}'"));
                        last_seen_text = text.clone();
                        if let Some(shortened) = process_clipboard_text(text, &state_monitor) {
                            last_seen_text = shortened;
                        }
                    }
                }
                Err(e) => {
                    log_debug(&format!("Clipboard poll error: {e:?}"));
                }
            }
        }
    });
}
