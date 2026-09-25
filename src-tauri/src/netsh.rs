//! Windows join via `netsh wlan`: save a per-user WLAN profile for the network,
//! then connect with it. Needs no administrator rights.
//!
//! Profile building and output parsing are plain functions compiled on every
//! platform so their tests run in the macOS CI job too; only the parts that
//! invoke netsh are Windows-only.
#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
use crate::process::run_with_timeout;

/// Upper bound on a single netsh invocation.
#[cfg(windows)]
const NETSH_TIMEOUT: Duration = Duration::from_secs(20);

/// How long to wait for the interface to report the target SSID after
/// `netsh wlan connect` accepts the request.
#[cfg(windows)]
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[cfg(windows)]
const CONNECT_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Keeps each netsh call from flashing a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A finished join. `verified` is false when Windows accepted the connect
/// request but would not report the current SSID (Windows 11 24H2+ hides it
/// from apps without Location access).
pub struct NetshJoin {
    pub interface: String,
    pub verified: bool,
}

/// Join `ssid` by saving a WLAN profile for the current user and connecting
/// with it. `security` is one of the normalized values "wpa", "wep", "nopass".
#[cfg(windows)]
pub fn join_via_netsh(
    ssid: &str,
    security: &str,
    password: Option<&str>,
    hidden: bool,
    progress: &dyn Fn(&str),
) -> Result<NetshJoin, String> {
    if ssid.contains('"') {
        return Err("Network names containing '\"' are not supported on Windows.".to_string());
    }

    if let Some(status) = interface_status() {
        if status.ssids.iter().any(|current| current == ssid) {
            log::info!("Already connected to '{ssid}'");
            return Ok(NetshJoin {
                interface: status.interface,
                verified: true,
            });
        }
    }

    let security = ProfileSecurity::from_normalized(security);
    let key = match security.key_type {
        Some(_) => {
            Some(password.ok_or_else(|| "Password is required for secured networks.".to_string())?)
        }
        None => None,
    };

    progress(&format!("Saving the Wi-Fi profile for '{ssid}'..."));
    add_profile(&profile_xml(ssid, &security, key, hidden))?;

    progress(&format!("Connecting to '{ssid}'..."));
    if let Err(error) = netsh(&["wlan", "connect", &format!("name={ssid}")]) {
        delete_profile(ssid);
        return Err(error);
    }

    match wait_until_connected(ssid) {
        ConnectOutcome::Connected(interface) => Ok(NetshJoin {
            interface,
            verified: true,
        }),
        ConnectOutcome::Unverifiable => Ok(NetshJoin {
            interface: "Wi-Fi".to_string(),
            verified: false,
        }),
        ConnectOutcome::TimedOut => {
            // Don't leave a profile with a wrong password behind for Windows
            // to keep retrying in the background.
            delete_profile(ssid);
            let hint = if key.is_some() {
                " Check that the password is correct."
            } else {
                ""
            };
            Err(format!("Could not join '{ssid}'.{hint}"))
        }
    }
}

#[cfg(windows)]
enum ConnectOutcome {
    Connected(String),
    Unverifiable,
    TimedOut,
}

#[cfg(windows)]
fn wait_until_connected(ssid: &str) -> ConnectOutcome {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        match interface_status() {
            None => return ConnectOutcome::Unverifiable,
            Some(status) if status.ssids.iter().any(|current| current == ssid) => {
                return ConnectOutcome::Connected(status.interface);
            }
            Some(_) => {}
        }
        if Instant::now() >= deadline {
            return ConnectOutcome::TimedOut;
        }
        std::thread::sleep(CONNECT_POLL_INTERVAL);
    }
}

/// Current Wi-Fi state, or `None` when netsh refuses to report it (e.g. no
/// Location access on Windows 11 24H2+).
#[cfg(windows)]
fn interface_status() -> Option<InterfaceStatus> {
    let output = netsh(&["wlan", "show", "interfaces"]).ok()?;
    if output.to_ascii_lowercase().contains("location") {
        return None;
    }
    Some(parse_interfaces(&output))
}

#[cfg(windows)]
fn add_profile(xml: &str) -> Result<(), String> {
    // The profile holds the password in plain text for the moment it takes
    // netsh to import it; it lives in the per-user temp dir and is removed
    // right after.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let path =
        std::env::temp_dir().join(format!("wilens-profile-{}-{nanos}.xml", std::process::id()));
    std::fs::write(&path, xml)
        .map_err(|error| format!("Failed to write Wi-Fi profile: {error}"))?;

    let result = netsh(&[
        "wlan",
        "add",
        "profile",
        &format!("filename={}", path.display()),
        "user=current",
    ]);
    let _ = std::fs::remove_file(&path);
    result.map(|_| ())
}

#[cfg(windows)]
fn delete_profile(ssid: &str) {
    if let Err(error) = netsh(&["wlan", "delete", "profile", &format!("name={ssid}")]) {
        log::warn!("Failed to remove Wi-Fi profile '{ssid}': {error}");
    }
}

/// Run netsh from System32 (not whatever `netsh` is first on PATH) and return
/// its stdout.
#[cfg(windows)]
fn netsh(args: &[&str]) -> Result<String, String> {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let program = std::path::Path::new(&system_root)
        .join("System32")
        .join("netsh.exe");

    let output = run_with_timeout(
        Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW),
        NETSH_TIMEOUT,
        "netsh",
    )?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() {
        return Ok(stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let details = if !stdout.is_empty() { stdout } else { stderr };
    Err(if details.is_empty() {
        "netsh failed without an error message.".to_string()
    } else {
        details
    })
}

/// WLAN profile security settings for one of the normalized QR security types.
pub struct ProfileSecurity {
    authentication: &'static str,
    encryption: &'static str,
    /// `None` for open networks, which carry no key.
    key_type: Option<&'static str>,
}

impl ProfileSecurity {
    pub fn from_normalized(security: &str) -> Self {
        match security {
            // WPA2-Personal also connects to WPA2/WPA3 transition networks.
            // WPA3-only networks need a WPA3SAE profile, which older Windows
            // releases reject, so they are not supported yet.
            "wpa" => Self {
                authentication: "WPA2PSK",
                encryption: "AES",
                key_type: Some("passPhrase"),
            },
            "wep" => Self {
                authentication: "open",
                encryption: "WEP",
                key_type: Some("networkKey"),
            },
            _ => Self {
                authentication: "open",
                encryption: "none",
                key_type: None,
            },
        }
    }
}

/// Build a WLAN profile XML document. The SSID is also given as hex so
/// non-ASCII names match the broadcast bytes exactly.
pub fn profile_xml(
    ssid: &str,
    security: &ProfileSecurity,
    key: Option<&str>,
    hidden: bool,
) -> String {
    let name = xml_escape(ssid);
    let hex: String = ssid.bytes().map(|byte| format!("{byte:02X}")).collect();
    let shared_key = match (security.key_type, key) {
        (Some(key_type), Some(key)) => format!(
            "\n            <sharedKey>\n                <keyType>{key_type}</keyType>\n                <protected>false</protected>\n                <keyMaterial>{}</keyMaterial>\n            </sharedKey>",
            xml_escape(key)
        ),
        _ => String::new(),
    };

    format!(
        r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
    <name>{name}</name>
    <SSIDConfig>
        <SSID>
            <hex>{hex}</hex>
            <name>{name}</name>
        </SSID>
        <nonBroadcast>{hidden}</nonBroadcast>
    </SSIDConfig>
    <connectionType>ESS</connectionType>
    <connectionMode>auto</connectionMode>
    <MSM>
        <security>
            <authEncryption>
                <authentication>{authentication}</authentication>
                <encryption>{encryption}</encryption>
                <useOneX>false</useOneX>
            </authEncryption>{shared_key}
        </security>
    </MSM>
</WLANProfile>
"#,
        authentication = security.authentication,
        encryption = security.encryption,
    )
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for char in value.chars() {
        match char {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(char),
        }
    }
    escaped
}

pub struct InterfaceStatus {
    /// Name of the first interface, e.g. "Wi-Fi".
    pub interface: String,
    /// SSIDs of every connected interface.
    pub ssids: Vec<String>,
}

/// Parse `netsh wlan show interfaces`. Lines are `Key : Value`; keys never
/// contain ':' but values (MAC addresses) do, so split on the first one.
/// "SSID" is not localized, unlike most other keys.
pub fn parse_interfaces(output: &str) -> InterfaceStatus {
    let mut interface = None;
    let mut ssids = Vec::new();

    for line in output.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "Name" if interface.is_none() => interface = Some(value.to_string()),
            "SSID" if !value.is_empty() => ssids.push(value.to_string()),
            _ => {}
        }
    }

    InterfaceStatus {
        interface: interface.unwrap_or_else(|| "Wi-Fi".to_string()),
        ssids,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_interfaces, profile_xml, xml_escape, ProfileSecurity};

    #[test]
    fn wpa_profile_has_passphrase_and_hex_ssid() {
        let xml = profile_xml(
            "Cafe",
            &ProfileSecurity::from_normalized("wpa"),
            Some("pw12345678"),
            false,
        );
        assert!(xml.contains("<hex>43616665</hex>"));
        assert!(xml.contains("<authentication>WPA2PSK</authentication>"));
        assert!(xml.contains("<keyType>passPhrase</keyType>"));
        assert!(xml.contains("<keyMaterial>pw12345678</keyMaterial>"));
        assert!(xml.contains("<nonBroadcast>false</nonBroadcast>"));
    }

    #[test]
    fn open_profile_has_no_shared_key() {
        let xml = profile_xml(
            "Guest",
            &ProfileSecurity::from_normalized("nopass"),
            None,
            true,
        );
        assert!(xml.contains("<encryption>none</encryption>"));
        assert!(!xml.contains("sharedKey"));
        assert!(xml.contains("<nonBroadcast>true</nonBroadcast>"));
    }

    #[test]
    fn wep_profile_uses_network_key() {
        let xml = profile_xml(
            "Old",
            &ProfileSecurity::from_normalized("wep"),
            Some("abcde"),
            false,
        );
        assert!(xml.contains("<encryption>WEP</encryption>"));
        assert!(xml.contains("<keyType>networkKey</keyType>"));
    }

    #[test]
    fn profile_escapes_xml_in_ssid_and_key() {
        let xml = profile_xml(
            "A&B <Home>",
            &ProfileSecurity::from_normalized("wpa"),
            Some("p\"w'<&>"),
            false,
        );
        assert!(xml.contains("<name>A&amp;B &lt;Home&gt;</name>"));
        assert!(xml.contains("<keyMaterial>p&quot;w&apos;&lt;&amp;&gt;</keyMaterial>"));
    }

    #[test]
    fn xml_escape_leaves_plain_text_alone() {
        assert_eq!(xml_escape("My Wifi 5G"), "My Wifi 5G");
    }

    #[test]
    fn parses_connected_interface() {
        let status = parse_interfaces(
            "There is 1 interface on the system:\r\n\r\n    Name                   : Wi-Fi\r\n    Physical address       : aa:bb:cc:dd:ee:ff\r\n    State                  : connected\r\n    SSID                   : Cafe: Upstairs\r\n    BSSID                  : 11:22:33:44:55:66\r\n",
        );
        assert_eq!(status.interface, "Wi-Fi");
        assert_eq!(status.ssids, vec!["Cafe: Upstairs".to_string()]);
    }

    #[test]
    fn parses_disconnected_interface() {
        let status = parse_interfaces(
            "There is 1 interface on the system:\r\n\r\n    Name                   : Wi-Fi 2\r\n    State                  : disconnected\r\n",
        );
        assert_eq!(status.interface, "Wi-Fi 2");
        assert!(status.ssids.is_empty());
    }
}
