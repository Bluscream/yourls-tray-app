use crate::config;
use std::thread;
use std::time::Duration;

pub fn fetch_history(config: &config::Config) -> Vec<(String, String)> {
    if config.servers.is_empty() {
        return Vec::new();
    }

    let mut handles = Vec::new();
    for server in &config.servers {
        let api_url = server.api_url.clone();
        let signature = server.signature.clone();
        if api_url.trim().is_empty() || signature.trim().is_empty() {
            continue;
        }

        let handle = thread::spawn(move || {
            let api_call_url = format!(
                "{}?signature={}&action=stats&filter=last&limit=25&format=json",
                api_url, signature
            );

            let response = match ureq::get(&api_call_url)
                .timeout(Duration::from_secs(3))
                .call()
            {
                Ok(res) => match res.into_string() {
                    Ok(s) => s,
                    Err(_) => return Vec::new(),
                },
                Err(_) => return Vec::new(),
            };

            let val: serde_json::Value = match serde_json::from_str(&response) {
                Ok(v) => v,
                Err(_) => return Vec::new(),
            };

            let mut server_history = Vec::new();
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
                    entries.sort_by(|a, b| b.0.cmp(&a.0));
                    server_history = entries.into_iter().map(|(_, l, s)| (l, s)).collect();
                }
            }
            server_history
        });
        handles.push((server.name.clone(), handle));
    }

    let mut all_histories = Vec::new();
    for (server_name, handle) in handles {
        if let Ok(server_history) = handle.join() {
            all_histories.push((server_name, server_history));
        }
    }

    let selected_server = config.selected_server.clone();
    let mut deduplicated = Vec::new();
    let mut seen_long = std::collections::HashSet::new();

    let mut selected_history = Vec::new();
    if selected_server != "Random" {
        if let Some((_, hist)) = all_histories.iter().find(|(name, _)| name == &selected_server) {
            selected_history = hist.clone();
        }
    }

    let mut idx = 0;
    let mut has_more = true;
    while has_more {
        has_more = false;
        for (_, hist) in &all_histories {
            if idx < hist.len() {
                has_more = true;
                let (long_url, short_url) = &hist[idx];
                if !seen_long.contains(long_url) {
                    seen_long.insert(long_url.clone());
                    
                    let mut final_short = short_url.clone();
                    if selected_server != "Random" {
                        if let Some((_, sel_short)) = selected_history.iter().find(|(l, _)| l == long_url) {
                            final_short = sel_short.clone();
                        }
                    }
                    deduplicated.push((long_url.clone(), final_short));
                }
            }
        }
        idx += 1;
    }

    deduplicated
}
