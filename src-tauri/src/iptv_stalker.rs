//! Stalker / Ministra ("MAG portal") Live TV logins.
//!
//! The portal identifies a set-top box by its MAC address. A session is a
//! handshake (which returns a bearer token), a profile call that activates the
//! token on most portals, then `itv` calls. Channel `cmd` values are not
//! playable on their own: each play asks the portal for a short-lived link
//! with `create_link`, which `iptv::resolve_live_stream` does.

use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

/// What a MAG box sends. Many portals refuse other agents.
pub const MAG_USER_AGENT: &str = "Mozilla/5.0 (QtEmbedded; U; Linux; C) AppleWebKit/533.3 (KHTML, like Gecko) MAG200 stbapp ver: 2 rev: 250 Safari/533.3";
const MAG_X_USER_AGENT: &str = "Model: MAG250; Link: WiFi";

/// One channel from `get_all_channels`.
#[derive(Debug, Clone, PartialEq)]
pub struct StalkerChannel {
    pub name: String,
    pub cmd: String,
    pub logo: Option<String>,
    pub group: Option<String>,
    pub epg_id: Option<String>,
}

/// The API endpoint for a portal address. `http://host/c/` uses
/// `/portal.php`, `http://host/stalker_portal/c/` uses the Ministra
/// `/stalker_portal/server/load.php`, and a URL already ending in `.php` is
/// used as given.
pub fn portal_api_url(portal: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(portal.trim())
        .map_err(|_| "Enter the portal URL starting with http:// or https://".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Enter the portal URL starting with http:// or https://".into());
    }
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().to_ascii_lowercase();
    if !path.ends_with(".php") {
        if path.contains("/stalker_portal") {
            url.set_path("/stalker_portal/server/load.php");
        } else {
            url.set_path("/portal.php");
        }
    }
    Ok(url)
}

/// Stream address inside a `cmd` such as `ffmpeg http://host/ch/1`.
pub fn strip_cmd_prefix(cmd: &str) -> &str {
    let cmd = cmd.trim();
    match cmd.find("http://").or_else(|| cmd.find("https://")) {
        Some(start) => cmd[start..].trim(),
        None => cmd,
    }
}

/// Responses wrap their payload in `{"js": …}`.
fn payload(body: &Value) -> &Value {
    body.get("js").unwrap_or(body)
}

fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub fn parse_token(body: &Value) -> Option<String> {
    text(payload(body).get("token"))
}

pub fn parse_genres(body: &Value) -> HashMap<String, String> {
    payload(body)
        .as_array()
        .map(|genres| {
            genres
                .iter()
                .filter_map(|genre| Some((text(genre.get("id"))?, text(genre.get("title"))?)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_channels(body: &Value, genres: &HashMap<String, String>) -> Vec<StalkerChannel> {
    let js = payload(body);
    let list = js.get("data").unwrap_or(js);
    list.as_array()
        .map(|channels| {
            channels
                .iter()
                .filter_map(|channel| {
                    let cmd = text(channel.get("cmd"))?;
                    let name = text(channel.get("name")).unwrap_or_else(|| "Channel".into());
                    let logo = text(channel.get("logo"))
                        .filter(|logo| logo.starts_with("http://") || logo.starts_with("https://"));
                    let group =
                        text(channel.get("tv_genre_id")).and_then(|id| genres.get(&id).cloned());
                    Some(StalkerChannel {
                        name,
                        cmd,
                        logo,
                        group,
                        epg_id: text(channel.get("xmltv_id")),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub struct StalkerSession {
    client: reqwest::Client,
    api: reqwest::Url,
    portal: String,
    mac: String,
    token: String,
    user_agent: String,
}

impl StalkerSession {
    /// Handshakes with the portal and activates the token.
    pub async fn connect(
        portal: &str,
        mac: &str,
        user_agent: Option<&str>,
    ) -> Result<Self, String> {
        let api = portal_api_url(portal)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| error.to_string())?;
        let mut session = Self {
            client,
            api,
            portal: portal.trim().to_string(),
            mac: mac.to_string(),
            token: String::new(),
            user_agent: user_agent.unwrap_or(MAG_USER_AGENT).to_string(),
        };
        let handshake = session
            .call(&[("type", "stb"), ("action", "handshake"), ("token", "")])
            .await?;
        session.token = parse_token(&handshake)
            .ok_or("The portal did not accept this MAC address. Check the portal URL and MAC.")?;
        // Most portals only honour the token after get_profile.
        session
            .call(&[("type", "stb"), ("action", "get_profile"), ("hd", "1")])
            .await?;
        Ok(session)
    }

    async fn call(&self, query: &[(&str, &str)]) -> Result<Value, String> {
        let mut request = self
            .client
            .get(self.api.clone())
            .query(query)
            .query(&[("JsHttpRequest", "1-xml")])
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header("X-User-Agent", MAG_X_USER_AGENT)
            .header(reqwest::header::REFERER, &self.portal)
            .header(
                reqwest::header::COOKIE,
                format!("mac={}; stb_lang=en; timezone=UTC", self.mac),
            );
        if !self.token.is_empty() {
            request = request.bearer_auth(&self.token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Could not reach the portal: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("The portal answered {}", response.status()));
        }
        let body = crate::iptv_relay::read_body_capped(response, 64 * 1024 * 1024)
            .await
            .map_err(|error| format!("Could not read the portal reply: {error}"))?;
        serde_json::from_slice(&body)
            .map_err(|_| "The portal sent a reply that is not a Stalker API response".to_string())
    }

    pub async fn channels(&self) -> Result<Vec<StalkerChannel>, String> {
        // Genre names are a nicety; channels still load without them.
        let genres = self
            .call(&[("type", "itv"), ("action", "get_genres")])
            .await
            .map(|body| parse_genres(&body))
            .unwrap_or_default();
        let body = self
            .call(&[("type", "itv"), ("action", "get_all_channels")])
            .await?;
        Ok(parse_channels(&body, &genres))
    }

    /// A playable address for a channel's `cmd`.
    pub async fn create_link(&self, cmd: &str) -> Result<String, String> {
        let body = self
            .call(&[
                ("type", "itv"),
                ("action", "create_link"),
                ("cmd", cmd),
                ("forced_storage", "undefined"),
                ("disable_ad", "0"),
            ])
            .await?;
        let link = text(payload(&body).get("cmd"))
            .ok_or("The portal did not return a stream link for this channel.")?;
        let url = strip_cmd_prefix(&link).to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("The portal returned a stream link the player cannot open.".into());
        }
        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;
    use axum::http::HeaderMap;
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;

    #[test]
    fn portal_urls_map_to_their_api_endpoint() {
        assert_eq!(
            portal_api_url("http://portal.example:8080/c/")
                .unwrap()
                .as_str(),
            "http://portal.example:8080/portal.php"
        );
        assert_eq!(
            portal_api_url("http://portal.example/stalker_portal/c/index.html")
                .unwrap()
                .as_str(),
            "http://portal.example/stalker_portal/server/load.php"
        );
        assert_eq!(
            portal_api_url("https://portal.example/server/load.php?x=1")
                .unwrap()
                .as_str(),
            "https://portal.example/server/load.php"
        );
        assert!(portal_api_url("ftp://portal.example/c/").is_err());
    }

    #[test]
    fn cmd_prefixes_are_stripped() {
        assert_eq!(
            strip_cmd_prefix("ffmpeg http://cdn.example/ch/1"),
            "http://cdn.example/ch/1"
        );
        assert_eq!(
            strip_cmd_prefix(" auto https://cdn.example/a.m3u8 "),
            "https://cdn.example/a.m3u8"
        );
        assert_eq!(
            strip_cmd_prefix("http://plain.example/1"),
            "http://plain.example/1"
        );
    }

    #[test]
    fn channels_parse_with_genres_and_skip_entries_without_cmd() {
        let genres = parse_genres(
            &json!({"js": [{"id": "5", "title": "News"}, {"id": 6, "title": "Sport"}]}),
        );
        let body = json!({"js": {"data": [
            {"id": "1", "name": "News One", "cmd": "ffmpeg http://localhost/ch/1_", "logo": "http://logo.example/1.png", "tv_genre_id": "5", "xmltv_id": "news.one"},
            {"id": "2", "name": "Sport Two", "cmd": "ffmpeg http://localhost/ch/2_", "logo": "2.png", "tv_genre_id": 6},
            {"id": "3", "name": "Broken"}
        ]}});
        let channels = parse_channels(&body, &genres);
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0],
            StalkerChannel {
                name: "News One".into(),
                cmd: "ffmpeg http://localhost/ch/1_".into(),
                logo: Some("http://logo.example/1.png".into()),
                group: Some("News".into()),
                epg_id: Some("news.one".into()),
            }
        );
        assert_eq!(channels[1].group.as_deref(), Some("Sport"));
        assert_eq!(channels[1].logo, None);
    }

    #[tokio::test]
    async fn session_handshakes_with_mac_and_token_then_creates_links() {
        async fn portal(
            Query(query): Query<HashMap<String, String>>,
            headers: HeaderMap,
        ) -> Json<Value> {
            let cookie = headers["cookie"].to_str().unwrap().to_string();
            assert!(cookie.contains("mac=00:1A:79:00:00:01"), "{cookie}");
            assert!(headers["user-agent"].to_str().unwrap().contains("MAG200"));
            let authorised = headers
                .get("authorization")
                .is_some_and(|value| value == "Bearer tok123");
            let reply = match (query["type"].as_str(), query["action"].as_str()) {
                ("stb", "handshake") => json!({"js": {"token": "tok123"}}),
                _ if !authorised => json!({"js": false}),
                ("stb", "get_profile") => json!({"js": {"id": 1}}),
                ("itv", "get_genres") => json!({"js": [{"id": "1", "title": "News"}]}),
                ("itv", "get_all_channels") => json!({"js": {"data": [
                    {"name": "News One", "cmd": "ffmpeg http://localhost/ch/1_", "tv_genre_id": "1"}
                ]}}),
                ("itv", "create_link") => {
                    assert_eq!(query["cmd"], "ffmpeg http://localhost/ch/1_");
                    json!({"js": {"cmd": "ffmpeg http://cdn.example/live/1.ts?play_token=abc"}})
                }
                _ => json!({"js": null}),
            };
            Json(reply)
        }
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, Router::new().route("/portal.php", get(portal))).await;
        });

        let session =
            StalkerSession::connect(&format!("http://{address}/c/"), "00:1A:79:00:00:01", None)
                .await
                .unwrap();
        let channels = session.channels().await.unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].group.as_deref(), Some("News"));
        assert_eq!(
            session.create_link(&channels[0].cmd).await.unwrap(),
            "http://cdn.example/live/1.ts?play_token=abc"
        );
    }
}
