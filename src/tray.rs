use crate::common::{AppState, log_debug};
use crate::config::{self, load_config};
use crate::i18n;
use std::sync::{Arc, Mutex};
use std::thread;
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, MenuId},
    MouseButton, TrayIconEvent,
};

pub fn create_icon(enabled: bool) -> tray_icon::Icon {
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

pub fn build_tray_menu(
    enabled: bool,
    history: &[(String, String)],
    bypass_double_copy: bool,
    bypass_shift_key: bool,
    bypass_scroll_lock: bool,
    enable_undo: bool,
    servers: &[config::ServerConfig],
    selected_server: &str,
    shorten_on_all: bool,
    locale: &str,
) -> (
    Menu,
    CheckMenuItem,
    MenuItem,
    MenuItem,
    CheckMenuItem,
    CheckMenuItem,
    CheckMenuItem,
    CheckMenuItem,
    std::collections::HashMap<MenuId, String>,
    CheckMenuItem,
    std::collections::HashMap<MenuId, String>,
    CheckMenuItem,
    MenuItem,
    MenuItem,
) {
    let resolved_locale = i18n::get_locale(locale);
    let menu = Menu::new();
    let current_version = env!("CARGO_PKG_VERSION");
    let title_text = format!("{} v{}", i18n::t(i18n::Key::AppTitle, &resolved_locale), current_version);
    let item_title = MenuItem::new(&title_text, true, None);
    let item_enabled = CheckMenuItem::new(i18n::t(i18n::Key::MonitorClipboard, &resolved_locale), true, enabled, None);
    let item_edit_config = MenuItem::new(i18n::t(i18n::Key::EditConfiguration, &resolved_locale), true, None);
    let item_check_update = MenuItem::new(i18n::t(i18n::Key::CheckForUpdates, &resolved_locale), true, None);
    let item_exit = MenuItem::new(i18n::t(i18n::Key::Exit, &resolved_locale), true, None);

    let select_server_submenu = Submenu::new(i18n::t(i18n::Key::SelectServer, &resolved_locale), true);
    let item_random = CheckMenuItem::new(i18n::t(i18n::Key::Random, &resolved_locale), true, selected_server == "Random", None);
    let _ = select_server_submenu.append(&item_random);

    let mut server_item_ids = std::collections::HashMap::new();
    for server in servers {
        let is_selected = selected_server == server.name;
        let item = CheckMenuItem::new(&server.name, true, is_selected, None);
        server_item_ids.insert(item.id().clone(), server.name.clone());
        let _ = select_server_submenu.append(&item);
    }

    let _ = select_server_submenu.append(&PredefinedMenuItem::separator());
    let item_shorten_all = CheckMenuItem::new(i18n::t(i18n::Key::ShortenOnAll, &resolved_locale), true, shorten_on_all, None);
    let _ = select_server_submenu.append(&item_shorten_all);

    let bypasses_submenu = Submenu::new(i18n::t(i18n::Key::BypassMethods, &resolved_locale), true);
    let item_bypass_double_copy = CheckMenuItem::new(i18n::t(i18n::Key::DoubleCopy, &resolved_locale), true, bypass_double_copy, None);
    let item_bypass_shift_key = CheckMenuItem::new(i18n::t(i18n::Key::ShiftKey, &resolved_locale), true, bypass_shift_key, None);
    let item_bypass_scroll_lock = CheckMenuItem::new(i18n::t(i18n::Key::ScrollLock, &resolved_locale), true, bypass_scroll_lock, None);
    let item_enable_undo = CheckMenuItem::new(i18n::t(i18n::Key::UndoHotkey, &resolved_locale), true, enable_undo, None);
    let _ = bypasses_submenu.append(&item_bypass_double_copy);
    let _ = bypasses_submenu.append(&item_bypass_shift_key);
    let _ = bypasses_submenu.append(&item_bypass_scroll_lock);
    let _ = bypasses_submenu.append(&item_enable_undo);

    let mut history_ids = std::collections::HashMap::new();
    let history_submenu = Submenu::new(i18n::t(i18n::Key::RecentLinks, &resolved_locale), true);

    if history.is_empty() {
        let no_links_item = MenuItem::new(i18n::t(i18n::Key::NoRecentLinks, &resolved_locale), false, None);
        let _ = history_submenu.append(&no_links_item);
    } else {
        for (long_url, short_url) in history {
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
        &select_server_submenu,
        &bypasses_submenu,
        &history_submenu,
        &PredefinedMenuItem::separator(),
        &item_check_update,
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
        item_enable_undo,
        history_ids,
        item_random,
        server_item_ids,
        item_shorten_all,
        item_check_update,
        item_title,
    )
}

pub fn handle_events(
    state: &Arc<Mutex<AppState>>,
    tray_icon: &tray_icon::TrayIcon,
    menu: &mut Menu,
    item_enabled: &mut CheckMenuItem,
    item_edit_config: &mut MenuItem,
    item_exit: &mut MenuItem,
    item_bypass_double_copy: &mut CheckMenuItem,
    item_bypass_shift_key: &mut CheckMenuItem,
    item_bypass_scroll_lock: &mut CheckMenuItem,
    item_enable_undo: &mut CheckMenuItem,
    history_ids: &mut std::collections::HashMap<MenuId, String>,
    item_random: &mut CheckMenuItem,
    server_item_ids: &mut std::collections::HashMap<MenuId, String>,
    item_shorten_all: &mut CheckMenuItem,
    item_check_update: &mut MenuItem,
    item_title: &mut MenuItem,
) {
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == item_title.id() {
            log_debug("Menu event: clicked title - opening GitHub repo");
            let locale = {
                let s = state.lock().unwrap();
                s.config.locale.clone()
            };
            let resolved = i18n::get_locale(&locale);
            let repo_url = format!("https://github.com/{}/{}",
                i18n::t(i18n::Key::GithubUser, &resolved),
                i18n::t(i18n::Key::GithubRepo, &resolved));
            crate::update::open_repo(&repo_url);
        } else if event.id == item_check_update.id() {
            log_debug("Menu event: clicked check for updates");
            let locale = {
                let s = state.lock().unwrap();
                s.config.locale.clone()
            };
            crate::update::check_for_updates(locale, true);
        } else if event.id == item_enabled.id() {
            let checked = item_enabled.is_checked();
            log_debug(&format!("Menu event: toggled enabled check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.enabled = checked;
            s.needs_menu_rebuild = true;
        } else if event.id == item_bypass_double_copy.id() {
            let checked = item_bypass_double_copy.is_checked();
            log_debug(&format!("Menu event: toggled bypass_double_copy check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.config.bypass_double_copy = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = s.config.clone();
            cfg.bypass_double_copy = checked;
            config::save_config(&cfg);
        } else if event.id == item_bypass_shift_key.id() {
            let checked = item_bypass_shift_key.is_checked();
            log_debug(&format!("Menu event: toggled bypass_shift_key check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.config.bypass_shift_key = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = s.config.clone();
            cfg.bypass_shift_key = checked;
            config::save_config(&cfg);
        } else if event.id == item_bypass_scroll_lock.id() {
            let checked = item_bypass_scroll_lock.is_checked();
            log_debug(&format!("Menu event: toggled bypass_scroll_lock check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.config.bypass_scroll_lock = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = s.config.clone();
            cfg.bypass_scroll_lock = checked;
            config::save_config(&cfg);
        } else if event.id == item_enable_undo.id() {
            let checked = item_enable_undo.is_checked();
            log_debug(&format!("Menu event: toggled enable_undo check state to {}", checked));
            let mut s = state.lock().unwrap();
            s.config.enable_undo = checked;
            s.needs_menu_rebuild = true;
            let mut cfg = s.config.clone();
            cfg.enable_undo = checked;
            config::save_config(&cfg);
        } else if event.id == item_random.id() {
            log_debug("Menu event: selected Random server.");
            let mut s = state.lock().unwrap();
            s.config.selected_server = "Random".to_string();
            s.needs_menu_rebuild = true;
            let cfg = s.config.clone();
            config::save_config(&cfg);
        } else if event.id == item_shorten_all.id() {
            let checked = item_shorten_all.is_checked();
            log_debug(&format!("Menu event: toggled shorten_on_all to {}", checked));
            let mut s = state.lock().unwrap();
            s.config.shorten_on_all = checked;
            s.needs_menu_rebuild = true;
            let cfg = s.config.clone();
            config::save_config(&cfg);
        } else if let Some(server_name) = server_item_ids.get(&event.id) {
            log_debug(&format!("Menu event: selected server '{}'.", server_name));
            let mut s = state.lock().unwrap();
            s.config.selected_server = server_name.clone();
            s.needs_menu_rebuild = true;
            let cfg = s.config.clone();
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
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if clipboard.set_text(short_url.clone()).is_ok() {
                        write_ok = true;
                    }
                }
            }
            #[cfg(target_os = "linux")]
            {
                if crate::clipboard::set_linux_clipboard(short_url).is_ok() {
                    write_ok = true;
                }
            }

            if write_ok {
                let locale = {
                    let s = state.lock().unwrap();
                    i18n::get_locale(&s.config.locale)
                };
                let body_text = i18n::t(i18n::Key::ShortLinkCopied, &locale)
                    .replace("{short_url}", &short_url)
                    .replace("{}", &short_url);
                let _ = notify_rust::Notification::new()
                    .summary(i18n::t(i18n::Key::CopiedFromHistory, &locale))
                    .body(&body_text)
                    .show();
            }
        }
    }

    while let Ok(event) = TrayIconEvent::receiver().try_recv() {
        if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
            let mut s = state.lock().unwrap();
            s.enabled = !s.enabled;
            s.needs_menu_rebuild = true;
            log_debug(&format!("Tray event: left-click toggled enabled status to {}", s.enabled));
        }
    }

    let rebuild = {
        let mut s = state.lock().unwrap();
        if s.needs_menu_rebuild {
            s.needs_menu_rebuild = false;
            s.config = load_config();
            Some((
                s.enabled,
                s.history.clone(),
                s.config.bypass_double_copy,
                s.config.bypass_shift_key,
                s.config.bypass_scroll_lock,
                s.config.enable_undo,
                s.config.servers.clone(),
                s.config.selected_server.clone(),
                s.config.shorten_on_all,
                s.config.locale.clone(),
            ))
        } else {
            None
        }
    };

    if let Some((enabled, history, bypass_double_copy, bypass_shift_key, bypass_scroll_lock, enable_undo, servers, selected_server, shorten_on_all, locale)) = rebuild {
        log_debug("Rebuilding context menu...");
        let (
            new_menu,
            new_enabled,
            new_edit,
            new_exit,
            new_bypass_double_copy,
            new_bypass_shift_key,
            new_bypass_scroll_lock,
            new_enable_undo,
            new_ids,
            new_random,
            new_server_item_ids,
            new_shorten_all,
            new_check_update,
            new_title,
        ) = build_tray_menu(
            enabled,
            &history,
            bypass_double_copy,
            bypass_shift_key,
            bypass_scroll_lock,
            enable_undo,
            &servers,
            &selected_server,
            shorten_on_all,
            &locale,
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
        *item_enable_undo = new_enable_undo;
        *history_ids = new_ids;
        *item_random = new_random;
        *server_item_ids = new_server_item_ids;
        *item_shorten_all = new_shorten_all;
        *item_check_update = new_check_update;
        *item_title = new_title;
    }
}

pub fn run_event_loop(
    state: Arc<Mutex<AppState>>,
    tray_icon: tray_icon::TrayIcon,
    mut menu: Menu,
    mut item_enabled: CheckMenuItem,
    mut item_edit_config: MenuItem,
    mut item_exit: MenuItem,
    mut item_bypass_double_copy: CheckMenuItem,
    mut item_bypass_shift_key: CheckMenuItem,
    mut item_bypass_scroll_lock: CheckMenuItem,
    mut item_enable_undo: CheckMenuItem,
    mut history_ids: std::collections::HashMap<MenuId, String>,
    mut item_random: CheckMenuItem,
    mut server_item_ids: std::collections::HashMap<MenuId, String>,
    mut item_shorten_all: CheckMenuItem,
    mut item_check_update: MenuItem,
    mut item_title: MenuItem,
) {
    #[cfg(target_os = "windows")]
    {
        let main_thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
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
                    &mut item_enable_undo,
                    &mut history_ids,
                    &mut item_random,
                    &mut server_item_ids,
                    &mut item_shorten_all,
                    &mut item_check_update,
                    &mut item_title,
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
                &mut item_enable_undo,
                &mut history_ids,
                &mut item_random,
                &mut server_item_ids,
                &mut item_shorten_all,
                &mut item_check_update,
                &mut item_title,
            );
            glib::ControlFlow::Continue
        });

        gtk::main();
    }
}
