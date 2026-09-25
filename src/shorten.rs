//! Turning a long URL into a short one.
//!
//! Extracted from the clipboard pipeline so the same code serves both the tray
//! and the command line. Nothing here touches the clipboard, the tray or any
//! global state: a URL and a configuration go in, a short URL comes out.

use crate::config::{Config, ServerConfig};
use std::time::Duration;
use url::Url;

/// How long to wait for a YOURLS instance to answer.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Why a URL could not be shortened.
#[derive(Debug)]
pub enum ShortenError {
    NotAUrl(String),
    /// The configured `blacklist_regex` is not a valid regular expression.
    BadBlacklist(String),
    NoServers,
    UnknownServer(String),
    IncompleteServer(String),
    Request {
        server: String,
        detail: String,
    },
    NotAUrlInReply {
        server: String,
        reply: String,
    },
}

impl std::fmt::Display for ShortenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAUrl(text) => write!(f, "not a URL: {text}"),
            Self::BadBlacklist(detail) => {
                write!(
                    f,
                    "blacklist_regex is not a valid regular expression: {detail}"
                )
            }
            Self::NoServers => write!(
                f,
                "no YOURLS server is configured; add one to {}",
                crate::config::get_config_path().display()
            ),
            Self::UnknownServer(name) => {
                write!(f, "no server named `{name}` is configured")
            }
            Self::IncompleteServer(name) => {
                write!(f, "server `{name}` is missing its api_url or signature")
            }
            Self::Request { server, detail } => write!(f, "{server}: {detail}"),
            Self::NotAUrlInReply { server, reply } => {
                write!(
                    f,
                    "{server} answered with something that is not a URL: {reply}"
                )
            }
        }
    }
}

impl std::error::Error for ShortenError {}

/// Checks that `text` is a URL worth shortening.
///
/// # Errors
///
/// Returns [`ShortenError::NotAUrl`] for anything that is not an absolute
/// `http(s)` URL — including a local file path, which is a URL but not one any
/// shortener can reach.
pub fn parse_url(text: &str) -> Result<Url, ShortenError> {
    let text = text.trim();
    let url = Url::parse(text).map_err(|_| ShortenError::NotAUrl(text.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ShortenError::NotAUrl(text.to_string()));
    }
    Ok(url)
}

/// Whether `url` is one the user asked never to shorten.
///
/// An empty `blacklist_regex` — the default — blacklists nothing. The pattern
/// is matched against the whole URL as written.
///
/// # Errors
///
/// Returns [`ShortenError::BadBlacklist`] if the pattern does not compile.
/// Refusing loudly is deliberate: treating an unparseable blacklist as "allow
/// everything" would quietly shorten exactly the links it was written to
/// protect.
pub fn is_blacklisted(url: &Url, config: &Config) -> Result<bool, ShortenError> {
    let pattern = config.blacklist_regex.trim();
    if pattern.is_empty() {
        return Ok(false);
    }
    let regex =
        regex::Regex::new(pattern).map_err(|e| ShortenError::BadBlacklist(e.to_string()))?;
    Ok(regex.is_match(url.as_str()))
}

/// The name that means "whatever the config says".
pub const AUTO: &str = "auto";

/// The server a request should go to.
///
/// `wanted` overrides the config for this one request: `None`, or `auto`,
/// leaves the choice to `selected_server`, which spreads links across every
/// configured instance unless it names one.
///
/// # Errors
///
/// Returns an error when nothing is configured, when the name given is not
/// configured, or when the chosen server is missing its credentials.
pub fn pick_server<'a>(
    config: &'a Config,
    wanted: Option<&str>,
) -> Result<&'a ServerConfig, ShortenError> {
    if config.servers.is_empty() {
        return Err(ShortenError::NoServers);
    }

    let selected = match wanted {
        None => config.selected_server.as_str(),
        Some(name) if name.eq_ignore_ascii_case(AUTO) => config.selected_server.as_str(),
        Some(name) => name,
    };

    let server = if selected.eq_ignore_ascii_case("random") {
        // Seeded from the clock: this only has to spread links out, so an
        // extra dependency for a better generator would not buy anything.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() as usize);
        &config.servers[nanos % config.servers.len()]
    } else {
        config
            .servers
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(selected))
            .ok_or_else(|| ShortenError::UnknownServer(selected.to_string()))?
    };

    if server.api_url.trim().is_empty() || server.signature.trim().is_empty() {
        return Err(ShortenError::IncompleteServer(server.name.clone()));
    }
    Ok(server)
}

/// Shortens `url`, and mirrors the resulting slug to the other servers when
/// `shorten_on_all` is set.
///
/// # Errors
///
/// Returns the reason the primary server could not be used or did not answer
/// with a URL. A failure to mirror to a *secondary* server is not an error:
/// the short URL the caller asked for already exists.
pub fn shorten(url: &Url, config: &Config, server: Option<&str>) -> Result<String, ShortenError> {
    let primary = pick_server(config, server)?;
    let agent = crate::common::get_agent(config.ignore_ssl_errors);
    let encoded: String = url::form_urlencoded::byte_serialize(url.as_str().as_bytes()).collect();

    let endpoint = format!(
        "{}?signature={}&action=shorturl&url={}&format=simple",
        primary.api_url, primary.signature, encoded
    );

    // The Ok and Status arms deliberately do the same thing: a URL this
    // instance already knows comes back as 409 with the existing short URL as
    // the body, which is the answer the caller wanted. Shortening the same
    // link twice should be idempotent, and ureq classifies every 4xx as an
    // error.
    #[allow(clippy::match_same_arms)]
    let response = match agent.get(&endpoint).timeout(TIMEOUT).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(e) => {
            return Err(ShortenError::Request {
                server: primary.name.clone(),
                // ureq quotes the whole request URL, signature included, and
                // this text goes to a terminal or a log.
                detail: crate::common::redact_secrets(&e.to_string()),
            });
        }
    };

    let reply = response
        .into_string()
        .map_err(|e| ShortenError::Request {
            server: primary.name.clone(),
            detail: crate::common::redact_secrets(&e.to_string()),
        })?
        .trim()
        .to_string();

    if !reply.starts_with("http://") && !reply.starts_with("https://") {
        return Err(ShortenError::NotAUrlInReply {
            server: primary.name.clone(),
            reply,
        });
    }

    if config.shorten_on_all {
        mirror(&reply, &encoded, primary, config, &agent);
    }
    Ok(reply)
}

/// Creates the same slug on every other configured server.
///
/// Best effort by design: the caller already has a working short URL, and a
/// second instance being down should not fail the whole request.
fn mirror(
    short_url: &str,
    encoded: &str,
    primary: &ServerConfig,
    config: &Config,
    agent: &ureq::Agent,
) {
    let slug = short_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    if slug.is_empty() {
        return;
    }

    for server in &config.servers {
        if server.name == primary.name
            || server.api_url.trim().is_empty()
            || server.signature.trim().is_empty()
        {
            continue;
        }
        let endpoint = format!(
            "{}?signature={}&action=shorturl&url={}&keyword={}&format=simple",
            server.api_url, server.signature, encoded, slug
        );
        let outcome = match agent.get(&endpoint).timeout(TIMEOUT).call() {
            Ok(_) => format!("mirrored {slug} to {}", server.name),
            Err(e) => format!(
                "could not mirror to {}: {}",
                server.name,
                crate::common::redact_secrets(&e.to_string())
            ),
        };
        crate::common::log_debug(&outcome);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str) -> ServerConfig {
        ServerConfig {
            name: name.to_string(),
            base_url: format!("https://{name}/"),
            api_url: format!("https://{name}/yourls-api.php"),
            signature: "secret".to_string(),
        }
    }

    #[test]
    fn only_http_urls_are_accepted() {
        assert!(parse_url("https://example.com/a").is_ok());
        assert!(parse_url("  http://example.com  ").is_ok());
        for bad in ["", "not a url", "file:///etc/passwd", "mailto:a@b.c"] {
            assert!(parse_url(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn a_named_server_is_used_and_an_unknown_one_is_reported() {
        let mut config = Config {
            servers: vec![server("one"), server("two")],
            selected_server: "two".to_string(),
            ..Config::default()
        };
        assert_eq!(pick_server(&config, None).expect("configured").name, "two");

        config.selected_server = "three".to_string();
        assert!(matches!(
            pick_server(&config, None),
            Err(ShortenError::UnknownServer(_))
        ));
    }

    #[test]
    fn an_asked_for_server_overrides_the_configured_one() {
        let config = Config {
            servers: vec![server("one"), server("two")],
            selected_server: "one".to_string(),
            ..Config::default()
        };
        assert_eq!(
            pick_server(&config, Some("two")).expect("configured").name,
            "two"
        );
        // Case-insensitive, because these are domain names.
        assert_eq!(
            pick_server(&config, Some("TWO")).expect("configured").name,
            "two"
        );
        assert!(matches!(
            pick_server(&config, Some("three")),
            Err(ShortenError::UnknownServer(_))
        ));
    }

    #[test]
    fn auto_and_nothing_both_defer_to_the_config() {
        let config = Config {
            servers: vec![server("one"), server("two")],
            selected_server: "two".to_string(),
            ..Config::default()
        };
        for wanted in [None, Some(AUTO), Some("AUTO")] {
            assert_eq!(
                pick_server(&config, wanted).expect("configured").name,
                "two",
                "{wanted:?} should follow the config"
            );
        }
    }

    #[test]
    fn a_blacklisted_url_is_recognised() {
        let config = Config {
            // The rule this was written for: Discord user links are already
            // short and shortening them breaks the client's preview.
            blacklist_regex: r"^https://discord\.com/users/\d{17,20}$".to_string(),
            ..Config::default()
        };
        let blacklisted = parse_url("https://discord.com/users/123456789012345678").expect("url");
        assert!(is_blacklisted(&blacklisted, &config).expect("valid pattern"));

        for allowed in [
            "https://discord.com/users/notanid",
            "https://example.com/users/123456789012345678",
            "https://discord.com/channels/1/2",
        ] {
            let url = parse_url(allowed).expect("url");
            assert!(
                !is_blacklisted(&url, &config).expect("valid pattern"),
                "{allowed} should not be blacklisted"
            );
        }
    }

    #[test]
    fn a_signature_never_reaches_an_error_message() {
        let text = "https://x/yourls-api.php?signature=deadbeef1234&action=shorturl";
        let redacted = crate::common::redact_secrets(text);
        assert!(!redacted.contains("deadbeef1234"), "{redacted}");
        assert!(redacted.contains("signature=<redacted>"), "{redacted}");
        assert!(redacted.contains("action=shorturl"), "{redacted}");
    }

    #[test]
    fn no_blacklist_blocks_nothing() {
        let config = Config::default();
        let url = parse_url("https://example.com").expect("url");
        assert!(!is_blacklisted(&url, &config).expect("empty is valid"));
    }

    #[test]
    fn an_unparseable_blacklist_is_an_error_rather_than_allow_everything() {
        let config = Config {
            blacklist_regex: "([unclosed".to_string(),
            ..Config::default()
        };
        let url = parse_url("https://example.com").expect("url");
        assert!(matches!(
            is_blacklisted(&url, &config),
            Err(ShortenError::BadBlacklist(_))
        ));
    }

    #[test]
    fn random_picks_one_of_the_configured_servers() {
        let config = Config {
            servers: vec![server("one"), server("two")],
            ..Config::default()
        };
        let chosen = pick_server(&config, None).expect("configured");
        assert!(chosen.name == "one" || chosen.name == "two");
    }

    #[test]
    fn nothing_configured_says_where_to_configure_it() {
        let config = Config::default();
        let message = pick_server(&config, None).expect_err("none").to_string();
        assert!(
            message.contains("no YOURLS server is configured"),
            "{message}"
        );
        assert!(message.contains("config.toml"), "{message}");
    }

    #[test]
    fn a_server_without_credentials_is_refused_by_name() {
        let mut incomplete = server("one");
        incomplete.signature = String::new();
        let config = Config {
            servers: vec![incomplete],
            ..Config::default()
        };
        assert!(matches!(
            pick_server(&config, None),
            Err(ShortenError::IncompleteServer(name)) if name == "one"
        ));
    }
}
