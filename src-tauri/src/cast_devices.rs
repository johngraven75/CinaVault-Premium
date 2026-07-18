use mdns_sd::{ServiceDaemon, ServiceEvent};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastDevice {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub control_url: Option<String>,
}

fn clean_service_name(fullname: &str) -> String {
    fullname
        .split("._")
        .next()
        .unwrap_or(fullname)
        .replace("\\032", " ")
}

fn discover_mdns_devices() -> Result<Vec<CastDevice>, String> {
    let daemon = ServiceDaemon::new().map_err(|error| error.to_string())?;
    let services = [
        ("_googlecast._tcp.local.", "google_cast"),
        ("_airplay._tcp.local.", "airplay"),
        ("_raop._tcp.local.", "airplay"),
    ];

    let mut browsers = Vec::new();
    for (service_type, protocol) in services {
        let receiver = daemon
            .browse(service_type)
            .map_err(|error| format!("Unable to browse {service_type}: {error}"))?;
        browsers.push((service_type, protocol, receiver));
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut devices = HashMap::<String, CastDevice>::new();
    while Instant::now() < deadline {
        for (_, protocol, receiver) in &browsers {
            if let Ok(ServiceEvent::ServiceResolved(info)) =
                receiver.recv_timeout(Duration::from_millis(100))
            {
                if let Some(address) = info.get_addresses_v4().iter().next() {
                    let host = address.to_string();
                    let port = info.get_port();
                    let id = format!("{protocol}:{host}:{port}");
                    devices.entry(id.clone()).or_insert(CastDevice {
                        id,
                        name: clean_service_name(info.get_fullname()),
                        protocol: (*protocol).to_string(),
                        host,
                        port,
                        control_url: None,
                    });
                }
            }
        }
    }

    for (service_type, _, _) in browsers {
        let _ = daemon.stop_browse(service_type);
    }
    let _ = daemon.shutdown();
    Ok(devices.into_values().collect())
}

fn header_value<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response.lines().find_map(|line| {
        let (header, value) = line.split_once(':')?;
        header.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

fn xml_value(xml: &str, tag: &str) -> Option<String> {
    let pattern = format!(r"(?is)<(?:[^:>]+:)?{tag}[^>]*>(.*?)</(?:[^:>]+:)?{tag}>");
    Regex::new(&pattern)
        .ok()?
        .captures(xml)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_string())
}

fn absolute_control_url(location: &str, control_path: &str) -> Option<String> {
    if control_path.starts_with("http://") || control_path.starts_with("https://") {
        return Some(control_path.to_string());
    }
    let url = reqwest::Url::parse(location).ok()?;
    url.join(control_path).ok().map(|joined| joined.to_string())
}

fn discover_samsung_renderers() -> Vec<CastDevice> {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        return Vec::new();
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(600)));
    let query = concat!(
        "M-SEARCH * HTTP/1.1\r\n",
        "HOST: 239.255.255.250:1900\r\n",
        "MAN: \"ssdp:discover\"\r\n",
        "MX: 2\r\n",
        "ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n"
    );
    if socket
        .send_to(query.as_bytes(), "239.255.255.250:1900")
        .is_err()
    {
        return Vec::new();
    }

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut locations = HashMap::<String, ()>::new();
    let mut buffer = [0_u8; 8192];
    while Instant::now() < deadline {
        let Ok((size, _)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let response = String::from_utf8_lossy(&buffer[..size]);
        if let Some(location) = header_value(&response, "location") {
            locations.insert(location.to_string(), ());
        }
    }

    let mut devices = Vec::new();
    for location in locations.keys() {
        let Ok(response) = client.get(location).send() else {
            continue;
        };
        let Ok(xml) = response.text() else {
            continue;
        };
        if !xml.to_ascii_lowercase().contains("samsung")
            || !xml.contains("AVTransport")
        {
            continue;
        }
        let Some(control_path) = xml_value(&xml, "controlURL") else {
            continue;
        };
        let Some(control_url) = absolute_control_url(location, &control_path) else {
            continue;
        };
        let Ok(parsed) = reqwest::Url::parse(location) else {
            continue;
        };
        let Some(host) = parsed.host_str() else {
            continue;
        };
        let port = parsed.port_or_known_default().unwrap_or(80);
        devices.push(CastDevice {
            id: format!("samsung_smart_view:{host}:{port}"),
            name: xml_value(&xml, "friendlyName").unwrap_or_else(|| "Samsung TV".to_string()),
            protocol: "samsung_smart_view".to_string(),
            host: host.to_string(),
            port,
            control_url: Some(control_url),
        });
    }
    devices
}

#[tauri::command]
pub async fn discover_cast_devices() -> Result<Vec<CastDevice>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut devices = discover_mdns_devices()?;
        devices.extend(discover_samsung_renderers());
        devices.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
        devices.dedup_by(|left, right| left.id == right.id);
        Ok(devices)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn samsung_soap(control_url: &str, action: &str, body: &str) -> Result<(), String> {
    let envelope = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">{body}</u:{action}></s:Body></s:Envelope>"
    );
    let response = reqwest::blocking::Client::new()
        .post(control_url)
        .header(
            "SOAPACTION",
            format!("\"urn:schemas-upnp-org:service:AVTransport:1#{action}\""),
        )
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(envelope)
        .send()
        .map_err(|error| error.to_string())?;
    response
        .error_for_status()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn cast_to_samsung(device: &CastDevice, media_url: &str) -> Result<String, String> {
    let control_url = device
        .control_url
        .as_deref()
        .ok_or_else(|| "Samsung AVTransport control URL is unavailable".to_string())?;
    samsung_soap(
        control_url,
        "SetAVTransportURI",
        &format!(
            "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData></CurrentURIMetaData>",
            xml_escape(media_url)
        ),
    )?;
    samsung_soap(
        control_url,
        "Play",
        "<InstanceID>0</InstanceID><Speed>1</Speed>",
    )?;
    Ok("Samsung Smart View playback started".to_string())
}

fn cast_to_airplay(device: &CastDevice, media_url: &str) -> Result<String, String> {
    let endpoint = format!("http://{}:{}/play", device.host, device.port);
    let response = reqwest::blocking::Client::new()
        .post(endpoint)
        .header("Content-Type", "text/parameters")
        .body(format!(
            "Content-Location: {media_url}\r\nStart-Position: 0.000000\r\n"
        ))
        .send()
        .map_err(|error| error.to_string())?;
    response
        .error_for_status()
        .map_err(|error| error.to_string())?;
    Ok("Apple AirPlay playback started".to_string())
}

#[tauri::command]
pub async fn cast_media_to_device(
    device: CastDevice,
    media_url: String,
    _title: String,
    _content_type: String,
) -> Result<String, String> {
    if !media_url.starts_with("http://") && !media_url.starts_with("https://") {
        return Err("Casting requires media reachable through the CinaVault server URL".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || match device.protocol.as_str() {
        "samsung_smart_view" => cast_to_samsung(&device, &media_url),
        "airplay" => cast_to_airplay(&device, &media_url),
        "google_cast" => Err("Google Cast is handled by the existing Cast client".to_string()),
        protocol => Err(format!("Unsupported casting protocol: {protocol}")),
    })
    .await
    .map_err(|error| error.to_string())?
}
