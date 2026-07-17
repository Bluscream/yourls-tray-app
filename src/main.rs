#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod config;
mod common;
mod api;
mod clipboard;
mod tray;
mod i18n;

use config::load_config;
use common::{AppState, log_debug};
use api::fetch_history;
#[cfg(target_os = "windows")]
use clipboard::spawn_clipboard_monitor;
#[cfg(target_os = "linux")]
use clipboard::spawn_linux_clipboard_poll;
#[cfg(target_os = "windows")]
use arboard::Clipboard;
use tray::{build_tray_menu, create_icon, run_event_loop};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tray_icon::TrayIconBuilder;
use notify_rust::Notification;
use device_query::{DeviceQuery, DeviceState, Keycode};

fn spawn_undo_hotkey(state_undo: Arc<Mutex<AppState>>) {
    thread::spawn(move || {
        log_debug("Spawning Ctrl+Backspace undo hotkey thread...");
        let device_state = DeviceState::new();
        let mut was_pressed = false;
        loop {
            thread::sleep(Duration::from_millis(100));
            let keys = device_state.get_keys();
            let ctrl = keys.contains(&Keycode::LControl) || keys.contains(&Keycode::RControl);
            let backspace = keys.contains(&Keycode::Backspace);
            let pressed = ctrl && backspace;

            if pressed && !was_pressed {
                let enable_undo = {
                    let s = state_undo.lock().unwrap();
                    s.config.enable_undo
                };

                if !enable_undo {
                    was_pressed = pressed;
                    continue;
                }

                log_debug("Ctrl+Backspace detected: attempting undo last shortening.");
                let (undo_pair, config) = {
                    let s = state_undo.lock().unwrap();
                    (s.last_undo_pair.clone(), s.config.clone())
                };

                let locale = i18n::get_locale(&config.locale);

                if let Some((long_url, short_url)) = undo_pair {
                    log_debug(&format!("Undoing: deleting {} and restoring {}", short_url, long_url));

                    let keyword = short_url
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .to_string();

                    if !keyword.is_empty() {
                        for server in &config.servers {
                            if server.api_url.trim().is_empty() || server.signature.trim().is_empty() {
                                continue;
                            }
                            let delete_url = format!(
                                "{}?signature={}&action=delete&shorturl={}&format=json",
                                server.api_url, server.signature, keyword
                            );
                            match ureq::get(&delete_url).timeout(Duration::from_secs(3)).call() {
                                Ok(res) => {
                                    let body = res.into_string().unwrap_or_default();
                                    log_debug(&format!("Delete API response from '{}': {}", server.name, body));
                                }
                                Err(e) => {
                                    log_debug(&format!("Delete API request failed for '{}': {:?}", server.name, e));
                                }
                            }
                        }
                    }

                    {
                        let mut s = state_undo.lock().unwrap();
                        s.bypass_undo_write = true;
                        s.last_undo_pair = None;
                    }

                    #[cfg(target_os = "windows")]
                    {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            for _ in 0..5 {
                                if clipboard.set_text(long_url.clone()).is_ok() {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(50));
                            }
                        }
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = clipboard::set_linux_clipboard(&long_url);
                    }

                    let body_text = i18n::t(i18n::Key::DeletedRestored, &locale)
                        .replace("{short_url}", &short_url)
                        .replace("{long_url}", &long_url)
                        .replace("{}", &short_url)
                        .replacen("{}", &long_url, 1);

                    let _ = Notification::new()
                        .summary(i18n::t(i18n::Key::ShorteningUndone, &locale))
                        .body(&body_text)
                        .show();

                    log_debug(i18n::t(i18n::Key::UndoComplete, &locale));
                } else {
                    log_debug(i18n::t(i18n::Key::NoUndoPair, &locale));
                }
            }

            was_pressed = pressed;
        }
    });
}

fn main() {
    log_debug("Application started.");
    
    let instance = single_instance::SingleInstance::new("yourls-tray-app-single-instance-lock").unwrap();
    if !instance.is_single() {
        log_debug("Another instance is already running. Exiting.");
        eprintln!("Another instance is already running. Exiting.");
        std::process::exit(1);
    }

    #[cfg(target_os = "linux")]
    {
        gtk::init().expect("Failed to initialize GTK");
    }

    let config = load_config();

    log_debug("Fetching initial history...");
    let initial_history = fetch_history(&config);

    let (
        menu,
        item_enabled,
        item_edit_config,
        item_exit,
        item_bypass_double_copy,
        item_bypass_shift_key,
        item_bypass_scroll_lock,
        item_enable_undo,
        history_ids,
        item_random,
        server_item_ids,
        item_shorten_all,
    ) = build_tray_menu(
        config.enabled,
        &initial_history,
        config.bypass_double_copy,
        config.bypass_shift_key,
        config.bypass_scroll_lock,
        config.enable_undo,
        &config.servers,
        &config.selected_server,
        config.shorten_on_all,
        &config.locale,
    );

    log_debug("Initializing tray icon...");
    let mut tray_icon = None;
    for i in 0..5 {
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("YOURLS Clipboard Shortener")
            .with_icon(create_icon(config.enabled))
            .build()
        {
            Ok(icon) => {
                tray_icon = Some(icon);
                break;
            }
            Err(e) => {
                log_debug(&format!("Tray icon initialization failed (attempt {}/5): {:?}", i + 1, e));
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
    let tray_icon = tray_icon.expect("Failed to initialize tray icon after 5 attempts");
    log_debug("Tray icon initialized successfully.");

    let state = Arc::new(Mutex::new(AppState {
        enabled: config.enabled,
        config,
        last_attempted_long_url: None,
        last_undo_pair: None,
        history: initial_history,
        needs_menu_rebuild: false,
        bypass_undo_write: false,
    }));

    #[cfg(target_os = "windows")]
    {
        let main_thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        spawn_clipboard_monitor(state.clone(), main_thread_id);
    }
    #[cfg(target_os = "linux")]
    {
        spawn_linux_clipboard_poll(state.clone());
    }

    spawn_undo_hotkey(state.clone());

    run_event_loop(
        state,
        tray_icon,
        menu,
        item_enabled,
        item_edit_config,
        item_exit,
        item_bypass_double_copy,
        item_bypass_shift_key,
        item_bypass_scroll_lock,
        item_enable_undo,
        history_ids,
        item_random,
        server_item_ids,
        item_shorten_all,
    );
}
