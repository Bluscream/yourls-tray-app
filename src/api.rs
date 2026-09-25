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
        let ignore_ssl = config.ignore_ssl_errors;
        if api_url.trim().is_empty() || signature.trim().is_empty() {
            continue;
        }

        let handle = thread::spawn(move || {
            let api_call_url = format!(
                "{api_url}?signature={signature}&action=stats&filter=last&limit=25&format=json"
            );

            let agent = crate::common::get_agent(ignore_ssl);
            let response = match agent
                .get(&api_call_url)
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
            if let Some(links_val) = val.get("links")
                && let Some(obj) = links_val.as_object()
            {
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
                entries.sort_by_key(|(id, _, _)| std::cmp::Reverse(*id));
                server_history = entries.into_iter().map(|(_, l, s)| (l, s)).collect();
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
    if selected_server != "Random"
        && let Some((_, hist)) = all_histories
            .iter()
            .find(|(name, _)| name == &selected_server)
    {
        selected_history = hist.clone();
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
                    if selected_server != "Random"
                        && let Some((_, sel_short)) =
                            selected_history.iter().find(|(l, _)| l == long_url)
                    {
                        final_short = sel_short.clone();
                    }
                    deduplicated.push((long_url.clone(), final_short));
                }
            }
        }
        idx += 1;
    }

    deduplicated
}
