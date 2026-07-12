#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod config;

#[cfg(target_os = "windows")]
use arboard::Clipboard;
#[cfg(target_os = "windows")]
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
use notify_rust::Notification;
use device_query::{DeviceQuery, DeviceState, Keycode};

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetKeyState(nVirtKey: i32) -> i16;
}

fn is_scroll_lock_active() -> bool {
    #[cfg(target_os = "windows")]
    {
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

fn log_debug(msg: &str) {
    let mut log_path = std::env::temp_dir();
    log_path.push("yourls-tray-app.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        use std::io::Write;
        let _ = writeln!(f, "{}", msg);
    }
}

struct AppState {
    enabled: bool,
    last_written_url: Option<String>,
    last_attempted_long_url: Option<String>,
    history: Vec<(String, String)>,
    needs_menu_rebuild: bool,
    bypass_double_copy: bool,
    bypass_shift_key: bool,
    bypass_scroll_lock: bool,
}

#[cfg(target_os = "windows")]
struct ClipboardMonitor {
    state: Arc<Mutex<AppState>>,
    main_thread_id: u32,
}

#[cfg(target_os = "windows")]
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
fn get_linux_clipboard() -> Result<String, std::io::Error> {
    let output = std::process::Command::new("wl-paste")
        .output();
    
    match output {
        Ok(out) if out.status.success() => {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
        _ => {
            // Fallback to xclip if Wayland is not present or fails
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
fn set_linux_clipboard(text: &str) -> Result<(), std::io::Error> {
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
            // Fallback to xclip
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

fn process_clipboard_text(
    text: String,
    state_clone: &Arc<Mutex<AppState>>,
    #[cfg(target_os = "windows")] clipboard: &mut Clipboard,
    #[cfg(target_os = "windows")] main_thread_id: u32,
) -> Option<String> {
    let text = text.trim().to_string();
    
    // Check if it's a valid absolute network URL (longer than 1 char scheme, and not file://)
    let parsed_url = match Url::parse(&text) {
        Ok(url) => url,
        Err(_) => return None,
    };
    if parsed_url.scheme().len() <= 1 || parsed_url.scheme() == "file" {
        return None;
    }

    // Lock state to read history, enabled status, and bypass toggles
    let (enabled, last_written, last_attempted, bypass_double_copy, bypass_shift_key, bypass_scroll_lock) = {
        let s = state_clone.lock().unwrap();
        (
            s.enabled,
            s.last_written_url.clone(),
            s.last_attempted_long_url.clone(),
            s.bypass_double_copy,
            s.bypass_shift_key,
            s.bypass_scroll_lock,
        )
    };

    // Check if Shift key is pressed or Scroll Lock is active (if enabled)
    let shift_pressed = bypass_shift_key && {
        let device_state = DeviceState::new();
        let keys = device_state.get_keys();
        keys.contains(&Keycode::LShift) || keys.contains(&Keycode::RShift)
    };
    let scroll_lock_active = bypass_scroll_lock && is_scroll_lock_active();

    if shift_pressed || scroll_lock_active {
        log_debug(&format!("Bypassing URL shortening. Shift pressed: {}, Scroll Lock active: {}", shift_pressed, scroll_lock_active));
        return None;
    }

    log_debug(&format!("Clipboard URL detected: {}", text));

    // Reload configuration to ensure we use the latest settings
    let config = load_config();

    log_debug(&format!("Active status: enabled={}, api_url='{}', signature='{}'", enabled, config.api_url, config.signature));
    if !enabled || config.api_url.trim().is_empty() || config.signature.trim().is_empty() {
        log_debug("App is disabled or configuration (API URL / Signature) is incomplete.");
        return None;
    }

    // Check regex blacklist pattern
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

    // 1. Avoid infinite loops
    if let Some(ref last_w) = last_written {
        if text == *last_w {
            return None;
        }
    }

    // 2. Ignore if it already starts with the configured base_url
    if text.starts_with(&config.base_url) {
        log_debug("Ignoring URL because it is already a shortened link.");
        return None;
    }

    // 3. Bypass check: copy same URL twice consecutively to bypass
    if bypass_double_copy {
        if let Some(ref last_att) = last_attempted {
            if text == *last_att {
                log_debug("URL copied twice consecutively. Bypassing shortening.");
                let mut s = state_clone.lock().unwrap();
                s.last_attempted_long_url = None;
                return None;
            }
        }
    }

    // Update the last attempted long URL before we shorten
    {
        let mut s = state_clone.lock().unwrap();
        s.last_attempted_long_url = Some(text.clone());
    }

    // Call YOURLS API to shorten
    log_debug(&format!("Calling API to shorten URL: {}", text));
    let encoded_url: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
    let api_call_url = format!(
        "{}?signature={}&action=shorturl&url={}&format=simple",
        config.api_url, config.signature, encoded_url
    );

    let response = match ureq::get(&api_call_url).call() {
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

    // Verify response URL
    if !response.starts_with("http://") && !response.starts_with("https://") {
        log_debug("API response was not a valid URL.");
        return None;
    }

    // Write back to clipboard
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

    // Show Notification
    let _ = Notification::new()
        .summary("Clipboard Link Shortened")
        .body(&format!("Original: {}\nShortened: {}", text, response))
        .show();

    // Fetch updated history and trigger menu rebuild
    let updated_history = fetch_history(&config);
    {
        let mut s = state_clone.lock().unwrap();
        s.last_written_url = Some(response.clone());
        s.history = updated_history;
        s.needs_menu_rebuild = true;
    }

    // Wake up main thread message pump (Windows only)
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
    bypass_double_copy: bool,
    bypass_shift_key: bool,
    bypass_scroll_lock: bool,
) -> (
    Menu,
    CheckMenuItem,
    MenuItem,
    MenuItem,
    CheckMenuItem,
    CheckMenuItem,
    CheckMenuItem,
    std::collections::HashMap<MenuId, String>,
) {
    let menu = Menu::new();
    let item_title = MenuItem::new("YOURLS Shortener", false, None);
    let item_enabled = CheckMenuItem::new("Monitor Clipboard", true, enabled, None);
    let item_edit_config = MenuItem::new("Edit Configuration", true, None);
    let item_exit = MenuItem::new("Exit", true, None);

    let bypasses_submenu = Submenu::new("Bypass Methods", true);
    let item_bypass_double_copy = CheckMenuItem::new("Double-Copy", true, bypass_double_copy, None);
    let item_bypass_shift_key = CheckMenuItem::new("Shift Key", true, bypass_shift_key, None);
    let item_bypass_scroll_lock = CheckMenuItem::new("Scroll Lock", true, bypass_scroll_lock, None);
    let _ = bypasses_submenu.append(&item_bypass_double_copy);
    let _ = bypasses_submenu.append(&item_bypass_shift_key);
    let _ = bypasses_submenu.append(&item_bypass_scroll_lock);

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
        &bypasses_submenu,
        &history_submenu,
        &PredefinedMenuItem::separator(),
        &item_exit,
    ])
    .unwrap();

    (
        menu,
        item_enabled,
        item_edit_config,
        item_exit,
        item_bypass_double_copy,
        item_bypass_shift_key,
        item_bypass_scroll_lock,
        history_ids,
    )
}

fn handle_events(
    state: &Arc<Mutex<AppState>>,
    tray_icon: &tray_icon::TrayIcon,
    menu: &mut Menu,
    item_enabled: &mut CheckMenuItem,
    item_edit_config: &mut MenuItem,
    item_exit: &mut MenuItem,
    item_bypass_double_copy: &mut CheckMenuItem,
    item_bypass_shift_key: &mut CheckMenuItem,
    item_bypass_scroll_lock: &mut CheckMenuItem,
    history_ids: &mut std::collections::HashMap<MenuId, String>,
) {
    // Drain Menu Events
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == item_enabled.id() {
            let checked = item_enabled.is_checked();
            log_debug(&format!("Menu event: toggled enabled check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.enabled = checked;
            s.needs_menu_rebuild = true;
        } else if event.id == item_bypass_double_copy.id() {
            let checked = item_bypass_double_copy.is_checked();
            log_debug(&format!("Menu event: toggled bypass_double_copy check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.bypass_double_copy = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = load_config();
            cfg.bypass_double_copy = checked;
            config::save_config(&cfg);
        } else if event.id == item_bypass_shift_key.id() {
            let checked = item_bypass_shift_key.is_checked();
            log_debug(&format!("Menu event: toggled bypass_shift_key check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.bypass_shift_key = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = load_config();
            cfg.bypass_shift_key = checked;
            config::save_config(&cfg);
        } else if event.id == item_bypass_scroll_lock.id() {
            let checked = item_bypass_scroll_lock.is_checked();
            log_debug(&format!("Menu event: toggled bypass_scroll_lock check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.bypass_scroll_lock = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = load_config();
            cfg.bypass_scroll_lock = checked;
            config::save_config(&cfg);
        } else if event.id == item_edit_config.id() {
            log_debug("Menu event: Edit Configuration clicked.");
            let config_path = config::get_config_path();
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("notepad.exe")
                .arg(config_path)
                .spawn();
            #[cfg(not(target_os = "windows"))]
            let _ = std::process::Command::new("xdg-open")
                .arg(config_path)
                .spawn();
        } else if event.id == item_exit.id() {
            log_debug("Menu event: Exit clicked. Shutting down.");
            std::process::exit(0);
        } else if let Some(short_url) = history_ids.get(&event.id) {
            log_debug(&format!("Menu event: history item clicked (url = {}). Copying to clipboard.", short_url));
            let mut write_ok = false;
            #[cfg(target_os = "windows")]
            {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.set_text(short_url.clone()).is_ok() {
                        write_ok = true;
                    }
                }
            }
            #[cfg(target_os = "linux")]
            {
                if set_linux_clipboard(short_url).is_ok() {
                    write_ok = true;
                }
            }

            if write_ok {
                let _ = Notification::new()
                    .summary("Copied from History")
                    .body(&format!("Short link copied to clipboard:\n{}", short_url))
                    .show();
            }
        }
    }

    // Drain Tray Icon Events
    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
            let mut s = state.lock().unwrap();
            s.enabled = !s.enabled;
            s.needs_menu_rebuild = true;
            log_debug(&format!("Tray event: left-click toggled enabled status to {}", s.enabled));
        }
    }

    // Check if we need to rebuild the menu
    let rebuild = {
        let mut s = state.lock().unwrap();
        if s.needs_menu_rebuild {
            s.needs_menu_rebuild = false;
            Some((s.enabled, s.history.clone(), s.bypass_double_copy, s.bypass_shift_key, s.bypass_scroll_lock))
        } else {
            None
        }
    };

    if let Some((enabled, history, bypass_double_copy, bypass_shift_key, bypass_scroll_lock)) = rebuild {
        log_debug("Rebuilding context menu...");
        let (
            new_menu,
            new_enabled,
            new_edit,
            new_exit,
            new_bypass_double_copy,
            new_bypass_shift_key,
            new_bypass_scroll_lock,
            new_ids,
        ) = build_tray_menu(
            enabled,
            &history,
            bypass_double_copy,
            bypass_shift_key,
            bypass_scroll_lock,
        );
        
        let _ = tray_icon.set_menu(Some(Box::new(new_menu.clone())));
        let _ = tray_icon.set_icon(Some(create_icon(enabled)));

        *menu = new_menu;
        *item_enabled = new_enabled;
        *item_edit_config = new_edit;
        *item_exit = new_exit;
        *item_bypass_double_copy = new_bypass_double_copy;
        *item_bypass_shift_key = new_bypass_shift_key;
        *item_bypass_scroll_lock = new_bypass_scroll_lock;
        *history_ids = new_ids;
    }
}

fn main() {
    log_debug("Application started.");
    
    // Enforce single instance by binding to a local port
    let _lock_socket = match std::net::UdpSocket::bind("127.0.0.1:58293") {
        Ok(socket) => socket,
        Err(_) => {
            log_debug("Another instance is already running. Exiting.");
            eprintln!("Another instance is already running. Exiting.");
            std::process::exit(1);
        }
    };

    #[cfg(target_os = "linux")]
    {
        gtk::init().expect("Failed to initialize GTK");
    }

    let config = load_config();

    // Fetch initial history
    log_debug("Fetching initial history...");
    let initial_history = fetch_history(&config);

    // Build initial menu
    let (
        mut menu,
        mut item_enabled,
        mut item_edit_config,
        mut item_exit,
        mut item_bypass_double_copy,
        mut item_bypass_shift_key,
        mut item_bypass_scroll_lock,
        mut history_ids,
    ) = build_tray_menu(
        config.enabled,
        &initial_history,
        config.bypass_double_copy,
        config.bypass_shift_key,
        config.bypass_scroll_lock,
    );

    // Create shared application state
    let state = Arc::new(Mutex::new(AppState {
        enabled: config.enabled,
        last_written_url: None,
        last_attempted_long_url: None,
        history: initial_history,
        needs_menu_rebuild: false,
        bypass_double_copy: config.bypass_double_copy,
        bypass_shift_key: config.bypass_shift_key,
        bypass_scroll_lock: config.bypass_scroll_lock,
    }));

    // Initialize Tray Icon
    log_debug("Initializing tray icon...");
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_tooltip("YOURLS Clipboard Shortener")
        .with_icon(create_icon(config.enabled))
        .build()
        .unwrap();
    log_debug("Tray icon initialized successfully.");

    // Spawn Clipboard Monitor thread
    let state_monitor = state.clone();

    #[cfg(target_os = "windows")]
    {
        let main_thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        thread::spawn(move || {
            log_debug("Spawning Clipboard Monitor thread...");
            let monitor = ClipboardMonitor {
                state: state_monitor,
                main_thread_id,
            };
            let mut master = Master::new(monitor).expect("Failed to initialize clipboard listener");
            log_debug("Clipboard Master starting run loop");
            master.run().expect("Clipboard listener loop failed");
        });
    }

    #[cfg(target_os = "linux")]
    {
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

    // Run platform-specific event loop
    #[cfg(target_os = "windows")]
    {
        let main_thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        // Spawn thread to listen to MenuEvent and TrayIconEvent channels, waking up the main thread's GetMessageW loop
        thread::spawn(move || {
            loop {
                crossbeam_channel::select! {
                    recv(MenuEvent::receiver()) -> _ => {
                        unsafe {
                            windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                                main_thread_id,
                                windows_sys::Win32::UI::WindowsAndMessaging::WM_USER,
                                0,
                                0,
                            );
                        }
                    }
                    recv(TrayIconEvent::receiver()) -> _ => {
                        unsafe {
                            windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                                main_thread_id,
                                windows_sys::Win32::UI::WindowsAndMessaging::WM_USER,
                                0,
                                0,
                            );
                        }
                    }
                }
            }
        });

        log_debug("Running Win32 Message Pump...");
        unsafe {
            let mut msg = std::mem::zeroed();
            while windows_sys::Win32::UI::WindowsAndMessaging::GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                windows_sys::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                windows_sys::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);

                handle_events(
                    &state,
                    &tray_icon,
                    &mut menu,
                    &mut item_enabled,
                    &mut item_edit_config,
                    &mut item_exit,
                    &mut item_bypass_double_copy,
                    &mut item_bypass_shift_key,
                    &mut item_bypass_scroll_lock,
                    &mut history_ids,
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        log_debug("Running GTK Event Loop...");

        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            handle_events(
                &state,
                &tray_icon,
                &mut menu,
                &mut item_enabled,
                &mut item_edit_config,
                &mut item_exit,
                &mut item_bypass_double_copy,
                &mut item_bypass_shift_key,
                &mut item_bypass_scroll_lock,
                &mut history_ids,
            );
            glib::ControlFlow::Continue
        });

        gtk::main();
    }
}
