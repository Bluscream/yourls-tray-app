// The windows subsystem suppresses the console window a tray app must not
// show. It is applied only to a build that actually has a tray: a CLI-only
// build (--no-default-features) is an ordinary console program, which is what
// a shell expects.
//
// A tray-enabled build used from the command line gets its console back at
// runtime instead — see the `console` module below.
#![cfg_attr(
    all(target_os = "windows", feature = "tray"),
    windows_subsystem = "windows"
)]

#[cfg(feature = "tray")]
mod api;
#[cfg(feature = "tray")]
mod clipboard;
mod common;
mod config;
#[cfg(feature = "tray")]
mod i18n;
mod shorten;
#[cfg(feature = "tray")]
mod tray;
#[cfg(feature = "tray")]
mod update;

#[cfg(feature = "tray")]
use api::fetch_history;
#[cfg(all(target_os = "windows", feature = "tray"))]
use arboard::Clipboard;
#[cfg(all(target_os = "windows", feature = "tray"))]
use clipboard::spawn_clipboard_monitor;
#[cfg(all(target_os = "linux", feature = "tray"))]
use clipboard::spawn_linux_clipboard_poll;
#[cfg(feature = "tray")]
use common::{AppState, log_debug};
use config::load_config;
#[cfg(feature = "tray")]
use device_query::{DeviceQuery, DeviceState, Keycode};
#[cfg(feature = "tray")]
use notify_rust::Notification;
#[cfg(feature = "tray")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "tray")]
use std::thread;
#[cfg(feature = "tray")]
use std::time::Duration;
#[cfg(feature = "tray")]
use tray::{build_tray_menu, create_icon, run_event_loop};
#[cfg(feature = "tray")]
use tray_icon::TrayIconBuilder;

/// Getting output onto a console on Windows.
///
/// A windows-subsystem process starts with no console and no valid standard
/// handles, so `println!` goes nowhere — which is fine for the tray and
/// useless for the CLI. Attaching to the console of whichever shell launched
/// us, and pointing the standard handles at it, makes `yourls <url>` print
/// where the user is looking.
///
/// Redirection and pipes already work without any of this: the parent hands
/// down real handles, and the first branch below leaves them alone.
#[cfg(all(target_os = "windows", feature = "tray"))]
mod console {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
        STD_OUTPUT_HANDLE, SetStdHandle,
    };

    /// Attaches to the parent's console, if there is one and we have none.
    ///
    /// Silent by design: every failure here means "there is no console to
    /// write to", which is not an error — the output is redirected, or the
    /// program was double-clicked.
    pub fn attach_to_parent() {
        // A valid handle already means output is redirected to a file or a
        // pipe, and hijacking it would send the result somewhere the user did
        // not ask for.
        #[allow(unsafe_code)]
        let existing = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        if !existing.is_null() {
            return;
        }
        #[allow(unsafe_code)]
        let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        if attached == 0 {
            return;
        }

        // CONOUT$ and CONIN$ name the console just attached, whatever the
        // standard handles currently point at. Opening them as ordinary files
        // avoids a CreateFileW call and its argument list.
        redirect("CONOUT$", &[STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]);
        redirect("CONIN$", &[STD_INPUT_HANDLE]);
    }

    /// Points each of `handles` at `device`.
    fn redirect(device: &str, handles: &[u32]) {
        let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(device)
        else {
            return;
        };
        let raw = file.as_raw_handle();
        for handle in handles {
            #[allow(unsafe_code)]
            unsafe {
                SetStdHandle(*handle, raw.cast());
            }
        }
        // The handle now belongs to the process's standard handles, and
        // dropping the File would close it out from under them.
        std::mem::forget(file);
    }
}

/// The tray's own state handling predates the lint policy; see the note at the
/// top of `tray.rs`.
#[cfg(feature = "tray")]
#[allow(clippy::unwrap_used, clippy::too_many_lines)]
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
                    log_debug(&format!(
                        "Undoing: deleting {short_url} and restoring {long_url}"
                    ));

                    let keyword = short_url
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .to_string();

                    if !keyword.is_empty() {
                        for server in &config.servers {
                            if server.api_url.trim().is_empty()
                                || server.signature.trim().is_empty()
                            {
                                continue;
                            }
                            let delete_url = format!(
                                "{}?signature={}&action=delete&shorturl={}&format=json",
                                server.api_url, server.signature, keyword
                            );
                            let agent = crate::common::get_agent(config.ignore_ssl_errors);
                            match agent
                                .get(&delete_url)
                                .timeout(Duration::from_secs(3))
                                .call()
                            {
                                Ok(res) => {
                                    let body = res.into_string().unwrap_or_default();
                                    log_debug(&format!(
                                        "Delete API response from '{}': {}",
                                        server.name, body
                                    ));
                                }
                                Err(e) => {
                                    log_debug(&format!(
                                        "Delete API request failed for '{}': {:?}",
                                        server.name, e
                                    ));
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
                        .replacen("{}", &short_url, 1)
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

/// What the command line asked for.
enum Invocation {
    /// Shorten this URL and print the result. The URL comes from an argument,
    /// or from standard input when there is none; the server is the one named
    /// by `--server`, or whatever the config picks.
    Shorten {
        url: Option<String>,
        server: Option<String>,
        id: Option<String>,
    },
    Tray,
    Help,
    Version,
    /// Something was wrong with the arguments themselves.
    Invalid(String),
}

fn parse_arguments(arguments: &[String]) -> Invocation {
    let mut url = None;
    let mut server = None;
    let mut id = None;
    // Which flag is waiting for its value, if any.
    let mut wants: Option<&str> = None;

    for argument in arguments {
        match wants.take() {
            Some("--server") => {
                server = Some(argument.clone());
                continue;
            }
            Some("--id") => {
                id = Some(argument.clone());
                continue;
            }
            Some(_) | None => {}
        }
        match argument.as_str() {
            "--tray" | "-t" => return Invocation::Tray,
            "--help" | "-h" => return Invocation::Help,
            "--version" | "-V" => return Invocation::Version,
            // Accepted and ignored: it is what the shortening mode does
            // anyway, and it reads well in a config file that spells out what
            // the command is for.
            "--shorten" | "-s" => {}
            "--server" => wants = Some("--server"),
            // --keyword is the API's own name for it, --slug is what the
            // rest of the world calls the last part of a short URL.
            "--id" | "--keyword" | "--slug" => wants = Some("--id"),
            // Both spellings, because `--server=x` is what anyone writes in a
            // config file and silently treating it as the URL would be worse
            // than any error.
            other if other.starts_with("--server=") => {
                server = Some(other["--server=".len()..].to_string());
            }
            other if other.starts_with("--id=") => {
                id = Some(other["--id=".len()..].to_string());
            }
            other if other.starts_with("--keyword=") => {
                id = Some(other["--keyword=".len()..].to_string());
            }
            other if other.starts_with("--slug=") => {
                id = Some(other["--slug=".len()..].to_string());
            }
            other => url = Some(other.to_string()),
        }
    }

    match wants {
        Some("--server") => {
            return Invocation::Invalid("--server needs a name (or `auto`)".to_string());
        }
        Some(flag) => return Invocation::Invalid(format!("{flag} needs a value")),
        None => {}
    }
    Invocation::Shorten { url, server, id }
}

const USAGE: &str = "\
yourls — shorten a URL with a YOURLS instance

Usage:
  yourls <url>        shorten that URL and print the short one
  cmd | yourls        read the URL from standard input instead
  yourls --tray       run the clipboard tray app
  yourls --help       show this
  yourls --version    show the version

Options:
  --server <name>     use that configured server. `auto`, or leaving it out,
                      follows the config's own choice — which spreads links
                      across every configured server unless it names one.
  --id <id>           ask for a specific short id instead of a generated one,
                      so the result is <base>/<id>. Also spelled --keyword
                      (the API's own name) or --slug. Fails if the id is
                      taken, or if this URL already has a different one.

The short URL is printed on standard output and nothing else is, so this can
be used in a pipe. Errors go to standard error and exit with status 1.

Servers are configured in the config file; the path is shown when none is.";

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    let invocation = parse_arguments(&arguments);

    // Anything but the tray writes to standard output, so it needs a console.
    #[cfg(all(target_os = "windows", feature = "tray"))]
    if !matches!(invocation, Invocation::Tray) {
        console::attach_to_parent();
    }

    match invocation {
        #[cfg(feature = "tray")]
        Invocation::Tray => run_tray(),
        #[cfg(not(feature = "tray"))]
        Invocation::Tray => {
            eprintln!("yourls: this build has no tray (compiled with --no-default-features)");
            std::process::exit(2);
        }
        Invocation::Help => println!("{USAGE}"),
        Invocation::Version => println!("yourls {}", env!("CARGO_PKG_VERSION")),
        Invocation::Shorten { url, server, id } => {
            if let Err(e) = run_shorten(url.as_deref(), server.as_deref(), id.as_deref()) {
                eprintln!("yourls: {e}");
                std::process::exit(1);
            }
        }
        Invocation::Invalid(reason) => {
            eprintln!("yourls: {reason}");
            std::process::exit(2);
        }
    }
}

/// Shortens one URL and prints it.
fn run_shorten(
    argument: Option<&str>,
    server: Option<&str>,
    id: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = if let Some(url) = argument {
        url.to_string()
    } else {
        use std::io::{IsTerminal as _, Read as _};

        // Reading stdin is for `echo url | yourls`. When stdin is a terminal
        // there is nothing to read, and blocking in read_to_string until the
        // user happens to press Ctrl-D is indistinguishable from a hang — so
        // a bare `yourls` says what it wants instead of sitting there.
        if std::io::stdin().is_terminal() {
            eprintln!("yourls: no URL given\n");
            eprintln!("{USAGE}");
            std::process::exit(2);
        }

        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    let url = shorten::parse_url(&input)?;
    let config = load_config();

    // A blacklisted URL is not a failure: the user asked for this one to be
    // left alone. Printing it back unchanged keeps `yourls "$url"` usable in a
    // pipe or as an editor filter, where an error would lose the URL
    // altogether. The reason goes to stderr so it is visible but not part of
    // the output.
    if shorten::is_blacklisted(&url, &config)? {
        eprintln!("yourls: {url} matches blacklist_regex; left unchanged");
        println!("{url}");
        return Ok(());
    }

    let short = shorten::shorten(&url, &config, server, id)?;
    println!("{short}");
    Ok(())
}

/// The tray's own state handling predates the lint policy; see the note at the
/// top of `tray.rs`.
#[cfg(feature = "tray")]
#[allow(clippy::unwrap_used, clippy::too_many_lines)]
fn run_tray() {
    log_debug("Application started.");

    let instance =
        single_instance::SingleInstance::new("yourls-tray-app-single-instance-lock").unwrap();
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
        item_check_update,
        item_title,
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
        let tooltip = i18n::t(i18n::Key::AppTooltip, &i18n::get_locale(&config.locale));
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip(tooltip)
            .with_icon(create_icon(config.enabled))
            .build()
        {
            Ok(icon) => {
                tray_icon = Some(icon);
                break;
            }
            Err(e) => {
                log_debug(&format!(
                    "Tray icon initialization failed (attempt {}/5): {:?}",
                    i + 1,
                    e
                ));
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

    let (check_on_startup, locale) = {
        let s = state.lock().unwrap();
        (s.config.check_update_on_startup, s.config.locale.clone())
    };
    if check_on_startup {
        update::check_for_updates(locale, false);
    }

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
        item_check_update,
        item_title,
    );
}

#[cfg(test)]
mod tests {
    use super::{Invocation, parse_arguments};

    fn parse(arguments: &[&str]) -> Invocation {
        let owned: Vec<String> = arguments.iter().map(|a| (*a).to_string()).collect();
        parse_arguments(&owned)
    }

    #[test]
    fn a_bare_url_is_shortened_with_no_server_preference() {
        match parse(&["https://example.com"]) {
            Invocation::Shorten { url, server, id } => {
                assert_eq!(url.as_deref(), Some("https://example.com"));
                assert_eq!(server, None);
                assert_eq!(id, None);
            }
            _ => panic!("expected Shorten"),
        }
    }

    #[test]
    fn the_server_can_be_given_either_way_round() {
        for arguments in [
            &["--server", "sho.rt", "https://example.com"][..],
            &["https://example.com", "--server=sho.rt"][..],
        ] {
            match parse(arguments) {
                Invocation::Shorten { url, server, .. } => {
                    assert_eq!(url.as_deref(), Some("https://example.com"));
                    assert_eq!(server.as_deref(), Some("sho.rt"));
                }
                _ => panic!("expected Shorten for {arguments:?}"),
            }
        }
    }

    #[test]
    fn a_server_with_no_name_is_an_error_rather_than_a_url() {
        // The regression this guards: treating the missing value as the URL
        // would shorten the string "--server".
        assert!(matches!(
            parse(&["https://example.com", "--server"]),
            Invocation::Invalid(_)
        ));
    }

    #[test]
    fn a_url_that_looks_like_a_server_name_is_still_the_value() {
        match parse(&["--server", "--tray"]) {
            Invocation::Shorten { server, .. } => assert_eq!(server.as_deref(), Some("--tray")),
            _ => panic!("the value after --server is a value, not a flag"),
        }
    }

    #[test]
    fn an_id_can_be_given_either_way_round_and_under_both_names() {
        for arguments in [
            &["--id", "mine", "https://example.com"][..],
            &["https://example.com", "--id=mine"][..],
            &["--keyword", "mine", "https://example.com"][..],
            &["https://example.com", "--keyword=mine"][..],
            &["--slug", "mine", "https://example.com"][..],
            &["https://example.com", "--slug=mine"][..],
        ] {
            match parse(arguments) {
                Invocation::Shorten { url, id, .. } => {
                    assert_eq!(url.as_deref(), Some("https://example.com"));
                    assert_eq!(id.as_deref(), Some("mine"), "{arguments:?}");
                }
                _ => panic!("expected Shorten for {arguments:?}"),
            }
        }
    }

    #[test]
    fn an_id_and_a_server_can_be_given_together() {
        match parse(&["--server", "a.de", "--id", "mine", "https://example.com"]) {
            Invocation::Shorten { url, server, id } => {
                assert_eq!(url.as_deref(), Some("https://example.com"));
                assert_eq!(server.as_deref(), Some("a.de"));
                assert_eq!(id.as_deref(), Some("mine"));
            }
            _ => panic!("expected Shorten"),
        }
    }

    #[test]
    fn an_id_with_no_value_is_an_error() {
        assert!(matches!(
            parse(&["https://example.com", "--id"]),
            Invocation::Invalid(_)
        ));
    }

    #[test]
    fn the_modes_are_recognised() {
        assert!(matches!(parse(&["--tray"]), Invocation::Tray));
        assert!(matches!(parse(&["-t"]), Invocation::Tray));
        assert!(matches!(parse(&["--help"]), Invocation::Help));
        assert!(matches!(parse(&["--version"]), Invocation::Version));
    }

    #[test]
    fn no_arguments_reads_standard_input() {
        match parse(&[]) {
            Invocation::Shorten { url, server, id } => {
                assert_eq!(url, None);
                assert_eq!(server, None);
                assert_eq!(id, None);
            }
            _ => panic!("expected Shorten"),
        }
    }
}
