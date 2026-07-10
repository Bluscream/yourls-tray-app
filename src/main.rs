#![windows_subsystem = "windows"]

mod config;

use arboard::Clipboard;
use clipboard_master::{CallbackResult, ClipboardHandler, Master};
use config::load_config;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, MenuId},
    MouseButton, TrayIconBuilder, TrayIconEvent,
};
use url::Url;
use winrt_notification::Toast;

struct AppState {
    enabled: bool,
    last_written_url: Option<String>,
    last_attempted_long_url: Option<String>,
    history: Vec<(String, String)>,
    needs_menu_rebuild: bool,
}

struct ClipboardMonitor {
    state: Arc<Mutex<AppState>>,
    main_thread_id: u32,
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        // Sleep briefly to ensure the copying app has finished releasing the clipboard lock.
        thread::sleep(Duration::from_millis(100));

        let state_clone = self.state.clone();
        let main_thread_id = self.main_thread_id;

        thread::spawn(move || {
            let mut clipboard = match Clipboard::new() {
                Ok(c) => c,
                Err(_) => return,
            };

            let text = match clipboard.get_text() {
                Ok(t) => t.trim().to_string(),
                Err(_) => return,
            };

            // Check if it's a valid absolute URL
            if Url::parse(&text).is_err() {
                return;
            }

            // Reload configuration to ensure we use the latest settings
            let config = load_config();

            // Lock state to read history and check if enabled
            let (enabled, last_written, last_attempted) = {
                let s = state_clone.lock().unwrap();
                (s.enabled, s.last_written_url.clone(), s.last_attempted_long_url.clone())
            };

            if !enabled || config.api_url.trim().is_empty() || config.signature.trim().is_empty() {
                return;
            }

            // 1. Avoid infinite loops
            if let Some(ref last_w) = last_written {
                if text == *last_w {
                    return;
                }
            }

            // 2. Ignore if it already starts with the configured base_url
            if text.starts_with(&config.base_url) {
                return;
            }

            // 3. Bypass check: copy same URL twice consecutively to bypass
            if let Some(ref last_att) = last_attempted {
                if text == *last_att {
                    let mut s = state_clone.lock().unwrap();
                    s.last_attempted_long_url = None;
                    return;
                }
            }

            // Update the last attempted long URL before we shorten
            {
                let mut s = state_clone.lock().unwrap();
                s.last_attempted_long_url = Some(text.clone());
            }

            // Call YOURLS API to shorten
            let encoded_url: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
            let api_call_url = format!(
                "{}?signature={}&action=shorturl&url={}&format=simple",
                config.api_url, config.signature, encoded_url
            );

            let response = match ureq::get(&api_call_url).call() {
                Ok(res) => match res.into_string() {
                    Ok(s) => s.trim().to_string(),
                    Err(_) => return,
                },
                Err(_) => return,
            };

            // Verify response URL
            if !response.starts_with("http://") && !response.starts_with("https://") {
                return;
            }

            // Write back to clipboard
            for _ in 0..5 {
                if clipboard.set_text(response.clone()).is_ok() {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }

            // Show Toast Notification
            let _ = Toast::new("YOURLS.ClipboardShortener")
                .title("Clipboard Link Shortened")
                .text1(&format!("Original: {}", text))
                .text2(&format!("Shortened: {}", response))
                .sound(Some(winrt_notification::Sound::Default))
                .show();

            // Fetch updated history and trigger menu rebuild
            let updated_history = fetch_history(&config);
            {
                let mut s = state_clone.lock().unwrap();
                s.last_written_url = Some(response);
                s.history = updated_history;
                s.needs_menu_rebuild = true;
            }

            // Wake up main thread message pump
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                    main_thread_id,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_USER,
                    0,
                    0,
                );
            }
        });

        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, _error: std::io::Error) -> CallbackResult {
        CallbackResult::Next
    }
}

fn fetch_history(config: &config::Config) -> Vec<(String, String)> {
    if config.api_url.trim().is_empty() || config.signature.trim().is_empty() {
        return Vec::new();
    }
    let api_call_url = format!(
        "{}?signature={}&action=stats&filter=last&limit=25&format=json",
        config.api_url, config.signature
    );

    let mut history = Vec::new();
    let response = match ureq::get(&api_call_url).call() {
        Ok(res) => match res.into_string() {
            Ok(s) => s,
            Err(_) => return history,
        },
        Err(_) => return history,
    };

    let val: serde_json::Value = match serde_json::from_str(&response) {
        Ok(v) => v,
        Err(_) => return history,
    };

    if let Some(links_val) = val.get("links") {
        if let Some(obj) = links_val.as_object() {
            let mut entries: Vec<(u32, String, String)> = Vec::new();
            for (key, val) in obj {
                let id: u32 = key
                    .strip_prefix("link_")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if let (Some(l), Some(s)) = (
                    val.get("url").and_then(|v| v.as_str()),
                    val.get("shorturl").and_then(|v| v.as_str()),
                ) {
                    entries.push((id, l.to_string(), s.to_string()));
                }
            }
            // Sort descending (newest first)
            entries.sort_by(|a, b| b.0.cmp(&a.0));
            history = entries.into_iter().map(|(_, l, s)| (l, s)).collect();
        }
    }

    history
}

fn create_icon(enabled: bool) -> tray_icon::Icon {
    let width = 32;
    let height = 32;
    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 4;
            let dx = (x as f32 - 15.5) / 13.0;
            let dy = (y as f32 - 15.5) / 13.0;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= 1.0 {
                let alpha = if dist > 0.9 {
                    ((1.0 - dist) * 10.0 * 255.0) as u8
                } else {
                    255
                };

                if enabled {
                    rgba[idx] = 41;
                    rgba[idx + 1] = 121;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = alpha;
                } else {
                    rgba[idx] = 120;
                    rgba[idx + 1] = 120;
                    rgba[idx + 2] = 120;
                    rgba[idx + 3] = alpha;
                }

                let x_i = x as i32;
                let y_i = y as i32;
                if (x_i - y_i).abs() < 3 && x_i > 9 && x_i < 23 {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                }
            } else {
                rgba[idx + 3] = 0;
            }
        }
    }
    tray_icon::Icon::from_rgba(rgba, width as u32, height as u32).unwrap()
}

fn build_tray_menu(
    enabled: bool,
    history: &[(String, String)],
) -> (
    Menu,
    CheckMenuItem,
    MenuItem,
    MenuItem,
    std::collections::HashMap<MenuId, String>,
) {
    let menu = Menu::new();
    let item_title = MenuItem::new("YOURLS Clipboard Shortener", false, None);
    let item_enabled = CheckMenuItem::new("Enabled", enabled, true, None);
    let item_edit_config = MenuItem::new("Edit Configuration", true, None);
    let item_exit = MenuItem::new("Exit", true, None);

    let mut history_ids = std::collections::HashMap::new();
    let history_submenu = Submenu::new("Recent Links", true);

    if history.is_empty() {
        let no_links_item = MenuItem::new("(no recent links)", false, None);
        let _ = history_submenu.append(&no_links_item);
    } else {
        for (long_url, short_url) in history {
            // Truncate long URLs to keep the menu clean
            let display_text = if long_url.len() > 50 {
                format!("{}...", &long_url[..47])
            } else {
                long_url.clone()
            };
            let item = MenuItem::new(display_text, true, None);
            history_ids.insert(item.id().clone(), short_url.clone());
            let _ = history_submenu.append(&item);
        }
    }

    menu.append_items(&[
        &item_title,
        &PredefinedMenuItem::separator(),
        &item_enabled,
        &item_edit_config,
        &history_submenu,
        &PredefinedMenuItem::separator(),
        &item_exit,
    ])
    .unwrap();

    (menu, item_enabled, item_edit_config, item_exit, history_ids)
}

fn main() {
    let config = load_config();

    // Fetch initial history
    let initial_history = fetch_history(&config);

    // Build initial menu
    let (mut _menu, mut item_enabled, mut item_edit_config, mut item_exit, mut history_ids) =
        build_tray_menu(config.enabled, &initial_history);

    // Create shared application state
    let state = Arc::new(Mutex::new(AppState {
        enabled: config.enabled,
        last_written_url: None,
        last_attempted_long_url: None,
        history: initial_history,
        needs_menu_rebuild: false,
    }));

    // Initialize Tray Icon
    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("YOURLS Clipboard Shortener")
        .with_icon(create_icon(config.enabled))
        .build()
        .unwrap();

    // Get current thread ID (main thread) so we can wake it up from the event listener thread
    let main_thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };

    // Create custom channels to safely forward menu and tray events to the main thread
    let (tx_menu, rx_menu) = crossbeam_channel::unbounded();
    let (tx_tray, rx_tray) = crossbeam_channel::unbounded();

    let tx_menu_clone = tx_menu.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = tx_menu_clone.send(event);
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                main_thread_id,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_USER,
                0,
                0,
            );
        }
    }));

    let tx_tray_clone = tx_tray.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        let _ = tx_tray_clone.send(event);
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                main_thread_id,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_USER,
                0,
                0,
            );
        }
    }));

    // Spawn Clipboard Monitor thread
    let state_monitor = state.clone();
    thread::spawn(move || {
        let monitor = ClipboardMonitor {
            state: state_monitor,
            main_thread_id,
        };
        let mut master = Master::new(monitor).expect("Failed to initialize clipboard listener");
        master.run().expect("Clipboard listener loop failed");
    });

    // Run Win32 Message Pump
    unsafe {
        let mut msg = std::mem::zeroed();
        while windows_sys::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
            windows_sys::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);

            // Drain Menu Events
            while let Ok(event) = rx_menu.try_recv() {
                if event.id == item_enabled.id() {
                    let checked = item_enabled.is_checked();
                    let mut s = state.lock().unwrap();
                    s.enabled = checked;
                    let _ = tray_icon.set_icon(Some(create_icon(checked)));
                } else if event.id == item_edit_config.id() {
                    let config_path = config::get_config_path();
                    let _ = std::process::Command::new("notepad.exe")
                        .arg(config_path)
                        .spawn();
                } else if event.id == item_exit.id() {
                    std::process::exit(0);
                } else if let Some(short_url) = history_ids.get(&event.id) {
                    // Copy short url from history click
                    if let Ok(mut clipboard) = Clipboard::new() {
                        let _ = clipboard.set_text(short_url.clone());
                        let _ = Toast::new("YOURLS.ClipboardShortener")
                            .title("Copied from History")
                            .text1("Short link copied to clipboard:")
                            .text2(short_url)
                            .sound(Some(winrt_notification::Sound::Default))
                            .show();
                    }
                }
            }

            // Drain Tray Icon Events
            while let Ok(event) = rx_tray.try_recv() {
                match event {
                    TrayIconEvent::Click { button: MouseButton::Left, .. } => {
                        let mut s = state.lock().unwrap();
                        s.enabled = !s.enabled;
                        let checked = s.enabled;
                        item_enabled.set_checked(checked);
                        let _ = tray_icon.set_icon(Some(create_icon(checked)));
                    }
                    TrayIconEvent::Click { button: MouseButton::Right, .. } => {
                        use tray_icon::menu::ContextMenu;
                        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
                        let _ = _menu.show_context_menu_for_hwnd(hwnd as isize, None);
                    }
                    _ => {}
                }
            }

            // Check if we need to rebuild the menu
            let rebuild = {
                let mut s = state.lock().unwrap();
                if s.needs_menu_rebuild {
                    s.needs_menu_rebuild = false;
                    Some((s.enabled, s.history.clone()))
                } else {
                    None
                }
            };

            if let Some((enabled, history)) = rebuild {
                let (new_menu, new_enabled, new_edit, new_exit, new_ids) =
                    build_tray_menu(enabled, &history);
                
                let _ = tray_icon.set_icon(Some(create_icon(enabled)));

                _menu = new_menu;
                item_enabled = new_enabled;
                item_edit_config = new_edit;
                item_exit = new_exit;
                history_ids = new_ids;
            }
        }
    }
}
