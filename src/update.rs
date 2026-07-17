use std::thread;
use serde_json::Value;
use crate::common::log_debug;
use crate::i18n;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].split('-').next()?.parse().ok()?;
        Some((major, minor, patch))
    } else {
        None
    }
}

fn is_newer(latest: &str, current: &str) -> bool {
    if let (Some(l), Some(c)) = (parse_version(latest), parse_version(current)) {
        if l.0 != c.0 {
            return l.0 > c.0;
        }
        if l.1 != c.1 {
            return l.1 > c.1;
        }
        l.2 > c.2
    } else {
        latest != current
    }
}

#[cfg(target_os = "windows")]
fn show_message_box(title: &str, text: &str, is_yes_no: bool) -> bool {
    use std::os::windows::ffi::OsStrExt;
    let title_wide: Vec<u16> = std::ffi::OsStr::new(title).encode_wide().chain(Some(0)).collect();
    let text_wide: Vec<u16> = std::ffi::OsStr::new(text).encode_wide().chain(Some(0)).collect();
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_YESNO, IDYES, MB_OK};
        let u_type = if is_yes_no {
            MB_YESNO | MB_ICONINFORMATION
        } else {
            MB_OK | MB_ICONINFORMATION
        };
        let ret = MessageBoxW(std::ptr::null_mut(), text_wide.as_ptr(), title_wide.as_ptr(), u_type);
        if is_yes_no {
            ret == IDYES
        } else {
            true
        }
    }
}

#[cfg(target_os = "linux")]
fn show_message_box(title: &str, text: &str, is_yes_no: bool) -> bool {
    let mut cmd = std::process::Command::new("zenity");
    if is_yes_no {
        cmd.arg("--question");
    } else {
        cmd.arg("--info");
    }
    cmd.arg("--title").arg(title).arg("--text").arg(text);
    
    if let Ok(status) = cmd.status() {
        status.success()
    } else {
        let mut fallback = std::process::Command::new("kdialog");
        if is_yes_no {
            fallback.arg("--yesno");
        } else {
            fallback.arg("--msgbox");
        }
        fallback.arg(text).arg("--title").arg(title);
        if let Ok(status) = fallback.status() {
            status.success()
        } else {
            false
        }
    }
}

pub fn open_repo(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(url)
            .spawn();
    }
}

fn open_browser(url: &str) {
    open_repo(url);
}

pub fn check_for_updates(locale: String, show_up_to_date: bool) {
    thread::spawn(move || {
        log_debug("Checking for updates...");
        let resolved_locale = i18n::get_locale(&locale);
        let user = i18n::t(i18n::Key::GithubUser, &resolved_locale);
        let repo = i18n::t(i18n::Key::GithubRepo, &resolved_locale);
        let latest_release_url = format!("https://api.github.com/repos/{}/{}/releases/latest", user, repo);

        let response = match ureq::get(&latest_release_url)
            .set("User-Agent", &format!("yourls-tray-app/{}", CURRENT_VERSION))
            .call()
        {
            Ok(res) => match res.into_string() {
                Ok(s) => s,
                Err(e) => {
                    log_debug(&format!("Failed to read update response: {:?}", e));
                    return;
                }
            },
            Err(e) => {
                log_debug(&format!("Failed to fetch updates from GitHub: {:?}", e));
                if show_up_to_date {
                    show_message_box(
                        i18n::t(i18n::Key::UpdateCheckFailed, &resolved_locale), 
                        i18n::t(i18n::Key::UpdateCheckFailedMsg, &resolved_locale), 
                        false
                    );
                }
                return;
            }
        };

        let val: Value = match serde_json::from_str(&response) {
            Ok(v) => v,
            Err(e) => {
                log_debug(&format!("Failed to parse update JSON: {:?}", e));
                return;
            }
        };

        if let Some(tag_name) = val.get("tag_name").and_then(|v| v.as_str()) {
            let fallback_url = format!("https://github.com/{}/{}/releases/latest", user, repo);
            let html_url = val.get("html_url").and_then(|v| v.as_str()).unwrap_or(&fallback_url);
            if is_newer(tag_name, CURRENT_VERSION) {
                log_debug(&format!("New update available: {} (current: {})", tag_name, CURRENT_VERSION));
                let question = i18n::t(i18n::Key::NewUpdateAvailableMsg, &resolved_locale)
                    .replace("{tag_name}", tag_name)
                    .replace("{current_version}", CURRENT_VERSION)
                    .replace("{}", tag_name)
                    .replacen("{}", CURRENT_VERSION, 1);
                
                if show_message_box(
                    i18n::t(i18n::Key::NewUpdateAvailable, &resolved_locale), 
                    &question, 
                    true
                ) {
                    open_browser(html_url);
                }
            } else {
                log_debug("Application is up to date.");
                if show_up_to_date {
                    let message = i18n::t(i18n::Key::AppUpToDateMsg, &resolved_locale)
                        .replace("{current_version}", CURRENT_VERSION)
                        .replace("{}", CURRENT_VERSION);
                    show_message_box(
                        i18n::t(i18n::Key::AppUpToDate, &resolved_locale), 
                        &message, 
                        false
                    );
                }
            }
        }
    });
}
