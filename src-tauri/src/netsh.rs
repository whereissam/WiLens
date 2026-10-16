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
/// `prefer_wpa3` is set when the QR code declared WPA3 (`T:SAE`); it only
/// decides which profile to try first when the scan can't tell.
#[cfg(windows)]
pub fn join_via_netsh(
    ssid: &str,
    security: &str,
    password: Option<&str>,
    hidden: bool,
    prefer_wpa3: bool,
    progress: &dyn Fn(&str),
) -> Result<NetshJoin, String> {
    validate_profile_name(ssid)?;

    // Being on a network with this name does not prove it is the network the
    // QR code describes: anyone can broadcast any SSID. Disconnect and join
    // again through the profile, whose security settings Windows enforces.
    // Disconnecting first also stops the old link from passing verification.
    if let Some(status) = interface_status() {
        if status.ssids.iter().any(|current| current == ssid) {
            progress(&format!("Reconnecting to '{ssid}'..."));
            netsh(&["wlan", "disconnect"])
                .map_err(|error| format!("Could not reconnect to '{ssid}': {error}"))?;
        }
    }

    let key = if security == "nopass" {
        None
    } else {
        Some(password.ok_or_else(|| "Password is required for secured networks.".to_string())?)
    };

    // A saved profile with this name belongs to the user (or an earlier
    // join). Never overwrite it with QR contents, and never delete it: connect
    // with it as-is and report failure without touching it.
    if profile_exists(ssid) {
        log::info!("Using the existing Wi-Fi profile for '{ssid}'");
        return match connect_and_wait(ssid, progress)? {
            ConnectOutcome::TimedOut => Err(format!(
                "Could not join '{ssid}' with the Wi-Fi profile already saved on this PC. WiLens left that profile unchanged; forget it in Windows Wi-Fi settings to use the scanned details instead."
            )),
            outcome => Ok(outcome.into_join()),
        };
    }

    let attempts = if security == "wpa" {
        wpa_attempts(prefer_wpa3, scan_security(ssid))
    } else {
        vec![ProfileSecurity::from_normalized(security)]
    };
    let mut wpa3_unsupported = false;

    for (index, profile) in attempts.iter().enumerate() {
        let is_last = index + 1 == attempts.len();
        if index > 0 {
            progress(&format!("Trying {} for '{ssid}'...", profile.label()));
        } else {
            progress(&format!("Saving the Wi-Fi profile for '{ssid}'..."));
        }

        // Windows releases before 10 version 2004, and some drivers, refuse a
        // WPA3SAE profile outright.
        if let Err(error) = add_profile(&profile_xml(ssid, profile, key, hidden)) {
            if profile.is_wpa3() {
                log::warn!("Could not save a WPA3 profile for '{ssid}': {error}");
                wpa3_unsupported = true;
                if !is_last {
                    continue;
                }
                return Err(format!(
                    "'{ssid}' needs WPA3, and this PC could not save a WPA3 Wi-Fi profile. WPA3 needs Windows 10 version 2004 or later and a Wi-Fi adapter that supports it."
                ));
            }
            return Err(error);
        }

        let outcome = match connect_and_wait(ssid, progress) {
            Ok(outcome) => outcome,
            Err(error) => {
                delete_profile(ssid);
                return Err(error);
            }
        };
        match outcome {
            ConnectOutcome::TimedOut => {
                // Don't leave a profile with a wrong password (or the wrong
                // security) behind for Windows to keep retrying.
                delete_profile(ssid);
            }
            outcome => return Ok(outcome.into_join()),
        }
    }

    let hint = match (key.is_some(), wpa3_unsupported) {
        (_, true) => " If the network is WPA3-only, this PC may not support WPA3.",
        (true, false) => " Check that the password is correct.",
        (false, false) => "",
    };
    Err(format!("Could not join '{ssid}'.{hint}"))
}

/// Profiles to try, in order, for a WPA-family QR code. A WPA2 profile also
/// joins WPA2/WPA3 transition networks; a WPA3-only network needs WPA3SAE.
pub fn wpa_attempts(prefer_wpa3: bool, scan: ScanSecurity) -> Vec<ProfileSecurity> {
    let wpa2 = ProfileSecurity::from_normalized("wpa");
    let wpa3 = ProfileSecurity::wpa3();
    match scan {
        ScanSecurity::Wpa3Only => vec![wpa3],
        ScanSecurity::Other => vec![wpa2],
        ScanSecurity::Unknown if prefer_wpa3 => vec![wpa3, wpa2],
        ScanSecurity::Unknown => vec![wpa2, wpa3],
    }
}

#[cfg(windows)]
fn connect_and_wait(ssid: &str, progress: &dyn Fn(&str)) -> Result<ConnectOutcome, String> {
    progress(&format!("Connecting to '{ssid}'..."));
    netsh(&["wlan", "connect", &format!("name={ssid}")])?;
    Ok(wait_until_connected(ssid))
}

/// What a Wi-Fi scan says about one network's security.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanSecurity {
    /// Every sighting advertises WPA3 and none WPA2 or older.
    Wpa3Only,
    /// Seen, and not WPA3-only.
    Other,
    /// Not in the scan, or the scan was refused (no Location access).
    Unknown,
}

#[cfg(windows)]
fn scan_security(ssid: &str) -> ScanSecurity {
    match netsh(&["wlan", "show", "networks"]) {
        Ok(output) if !is_location_refusal(&output) => parse_scan_security(&output, ssid),
        _ => ScanSecurity::Unknown,
    }
}

/// Parse `netsh wlan show networks` for `ssid`. Each network is an
/// `SSID <n> : <name>` line followed by indented `Key : Value` lines. The keys
/// are localized but the "SSID" label and the "WPA2"/"WPA3" values are not,
/// so only those are relied on.
pub fn parse_scan_security(output: &str, ssid: &str) -> ScanSecurity {
    let mut in_target = false;
    let mut seen = false;
    let mut has_wpa3 = false;
    let mut has_older = false;

    for line in output.lines() {
        let line = line.trim_end_matches('\r');
        let Some((key, value)) = line.split_once(" : ") else {
            continue;
        };
        // "BSSID n" lines don't match, so only network headers do.
        if key.trim().starts_with("SSID ") {
            in_target = value == ssid;
            seen |= in_target;
            continue;
        }
        if !in_target {
            continue;
        }
        let value = value.trim().to_ascii_uppercase();
        if value.contains("WPA3") {
            has_wpa3 = true;
        } else if value.starts_with("WPA") || value == "WEP" || value == "OPEN" {
            has_older = true;
        }
    }

    match (seen, has_wpa3, has_older) {
        (true, true, false) => ScanSecurity::Wpa3Only,
        (true, _, _) => ScanSecurity::Other,
        (false, _, _) => ScanSecurity::Unknown,
    }
}

#[cfg(windows)]
enum ConnectOutcome {
    Connected(String),
    Unverifiable,
    TimedOut,
}

#[cfg(windows)]
impl ConnectOutcome {
    /// The successful join for a `Connected` or `Unverifiable` outcome.
    fn into_join(self) -> NetshJoin {
        match self {
            ConnectOutcome::Connected(interface) => NetshJoin {
                interface,
                verified: true,
            },
            _ => NetshJoin {
                interface: "Wi-Fi".to_string(),
                verified: false,
            },
        }
    }
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
    if is_location_refusal(&output) {
        return None;
    }
    Some(parse_interfaces(&output))
}

/// Whether a profile with this name is already saved. Windows matches profile
/// names case-insensitively, so this does too.
#[cfg(windows)]
fn profile_exists(ssid: &str) -> bool {
    match netsh(&["wlan", "show", "profiles"]) {
        Ok(output) => parse_profile_names(&output)
            .iter()
            .any(|name| name.eq_ignore_ascii_case(ssid)),
        Err(error) => {
            log::warn!("Could not list saved Wi-Fi profiles: {error}");
            false
        }
    }
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
    // create_new refuses to follow or reuse anything already at this path.
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, xml.as_bytes()))
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

/// netsh treats '*' and '?' in a profile `name=` as wildcards ("delete profile
/// name=*" deletes every saved profile), and '"' would end the quoted
/// argument. Refuse them so the SSID is always a literal profile name.
pub fn validate_profile_name(ssid: &str) -> Result<(), String> {
    match ssid.chars().find(|char| matches!(char, '*' | '?' | '"')) {
        Some(char) => Err(format!(
            "Network names containing '{char}' are not supported on Windows."
        )),
        None => Ok(()),
    }
}

/// Whether `netsh wlan show interfaces` printed the Location-permission refusal
/// (Windows 11 24H2+) instead of interface details. Real status output is made
/// of "Key : Value" lines, so a connected SSID that happens to contain the word
/// "location" is not mistaken for the refusal.
pub fn is_location_refusal(output: &str) -> bool {
    output.to_ascii_lowercase().contains("location")
        && !output.lines().any(|line| line.contains(" : "))
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
            // WPA3-only networks need `wpa3()` instead.
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

    /// WPA3-Personal (SAE). Windows 10 version 2004 and later accept it.
    pub fn wpa3() -> Self {
        Self {
            authentication: "WPA3SAE",
            encryption: "AES",
            key_type: Some("passPhrase"),
        }
    }

    pub fn is_wpa3(&self) -> bool {
        self.authentication == "WPA3SAE"
    }

    fn label(&self) -> &'static str {
        if self.is_wpa3() {
            "WPA3"
        } else {
            "WPA2"
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

/// Parse `netsh wlan show profiles`. Each saved profile is a
/// `<localized label> : <name>` line; headers and "<None>" placeholders have no
/// " : ". Only the separator is removed so names keep their own spaces.
pub fn parse_profile_names(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.split_once(" : "))
        .map(|(_, name)| name.trim_end_matches('\r').to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

pub struct InterfaceStatus {
    /// Name of the first interface, e.g. "Wi-Fi".
    pub interface: String,
    /// SSIDs of every connected interface.
    pub ssids: Vec<String>,
}

/// Parse `netsh wlan show interfaces`. Lines are `Key : Value`; keys never
/// contain ':' but values (MAC addresses) do, so split on the first one.
/// "SSID" is not localized, unlike most other keys. Only the single space after
/// the colon is removed from SSIDs, so names with their own leading or trailing
/// spaces still match the requested SSID exactly.
pub fn parse_interfaces(output: &str) -> InterfaceStatus {
    let mut interface = None;
    let mut ssids = Vec::new();

    for line in output.lines() {
        let Some((key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let raw_value = raw_value.trim_end_matches('\r');
        match key.trim() {
            "Name" if interface.is_none() => interface = Some(raw_value.trim().to_string()),
            "SSID" => {
                let ssid = raw_value.strip_prefix(' ').unwrap_or(raw_value);
                if !ssid.is_empty() {
                    ssids.push(ssid.to_string());
                }
            }
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
    use super::{
        is_location_refusal, parse_interfaces, parse_profile_names, parse_scan_security,
        profile_xml, validate_profile_name, wpa_attempts, xml_escape, ProfileSecurity,
        ScanSecurity,
    };

    const SCAN: &str = "Interface name : Wi-Fi
There are 3 networks currently visible.

SSID 1 : Home
    Network type            : Infrastructure
    Authentication          : WPA3-Personal
    Encryption              : CCMP

SSID 2 : Cafe
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP

SSID 3 : Home 5G
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
";

    #[test]
    fn wpa3_profile_uses_sae() {
        let xml = profile_xml("Home", &ProfileSecurity::wpa3(), Some("pw12345678"), false);
        assert!(xml.contains("<authentication>WPA3SAE</authentication>"));
        assert!(xml.contains("<encryption>AES</encryption>"));
        assert!(xml.contains("<keyType>passPhrase</keyType>"));
    }

    #[test]
    fn reads_network_security_from_scan() {
        assert_eq!(parse_scan_security(SCAN, "Home"), ScanSecurity::Wpa3Only);
        assert_eq!(parse_scan_security(SCAN, "Cafe"), ScanSecurity::Other);
        assert_eq!(parse_scan_security(SCAN, "Home 5G"), ScanSecurity::Other);
        assert_eq!(parse_scan_security(SCAN, "Missing"), ScanSecurity::Unknown);
    }

    #[test]
    fn a_network_seen_with_wpa2_anywhere_is_not_wpa3_only() {
        let scan = "SSID 1 : Mesh
    Authentication : WPA3-Personal
SSID 2 : Mesh
    Authentication : WPA2-Personal
";
        assert_eq!(parse_scan_security(scan, "Mesh"), ScanSecurity::Other);
    }

    #[test]
    fn tries_profiles_in_the_right_order() {
        let order = |prefer, scan| {
            wpa_attempts(prefer, scan)
                .iter()
                .map(ProfileSecurity::is_wpa3)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(false, ScanSecurity::Wpa3Only), vec![true]);
        assert_eq!(order(true, ScanSecurity::Other), vec![false]);
        assert_eq!(order(false, ScanSecurity::Unknown), vec![false, true]);
        assert_eq!(order(true, ScanSecurity::Unknown), vec![true, false]);
    }

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
    fn keeps_spaces_that_belong_to_the_ssid() {
        let status = parse_interfaces(
            "    Name                   : Wi-Fi\r\n    SSID                   :  Padded \r\n",
        );
        assert_eq!(status.ssids, vec![" Padded ".to_string()]);
    }

    #[test]
    fn rejects_netsh_wildcards_and_quotes_in_profile_names() {
        assert!(validate_profile_name("*").is_err());
        assert!(validate_profile_name("Home*").is_err());
        assert!(validate_profile_name("Cafe?").is_err());
        assert!(validate_profile_name("Say \"hi\"").is_err());
        assert!(validate_profile_name("Cafe: Upstairs 5G").is_ok());
    }

    #[test]
    fn parses_saved_profile_names() {
        let names = parse_profile_names(
            "Profiles on interface Wi-Fi:\r\n\r\nGroup policy profiles (read only)\r\n---------------------------------\r\n    <None>\r\n\r\nUser profiles\r\n-------------\r\n    All User Profile     : Home\r\n    All User Profile     : Cafe: Upstairs\r\n    User Profile         :  Padded\r\n",
        );
        assert_eq!(names, vec!["Home", "Cafe: Upstairs", " Padded"]);
    }

    #[test]
    fn detects_only_the_real_location_refusal() {
        assert!(is_location_refusal(
            "Network shell commands need location permission to access WLAN information.\r\nTurn on Location services (ms-settings:privacy-location).\r\n"
        ));
        assert!(!is_location_refusal(
            "    Name                   : Wi-Fi\r\n    SSID                   : Location Cafe\r\n"
        ));
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
