use std::process::Command;
use std::time::{Duration, Instant};

use crate::process::run_with_timeout;

#[cfg(target_os = "macos")]
use objc2_core_location::{CLAuthorizationStatus, CLLocationManager};
#[cfg(target_os = "macos")]
use objc2_core_wlan::{CWInterface, CWNetwork, CWSecurity, CWWiFiClient};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSError, NSString};

/// Maximum number of associate attempts before giving up on CoreWLAN.
/// `associateToNetwork` can report success before the link is actually up; a
/// second associate is what reliably connects (the behavior users hit manually
/// as a "double click"), so we retry automatically.
#[cfg(target_os = "macos")]
const COREWLAN_MAX_ATTEMPTS: u32 = 3;

/// How long to wait for the interface to report the target SSID after an
/// associate call returns, before treating the attempt as not-yet-connected.
#[cfg(target_os = "macos")]
const COREWLAN_VERIFY_TIMEOUT: Duration = Duration::from_millis(2500);

/// Polling interval while verifying the connection.
#[cfg(target_os = "macos")]
const COREWLAN_VERIFY_INTERVAL: Duration = Duration::from_millis(300);

/// Upper bound on a `networksetup` invocation so a wedged process cannot leave
/// the join hanging forever.
const NETWORKSETUP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub struct HardwarePort {
    pub port: String,
    pub device: String,
}

/// Why a CoreWLAN join did not succeed, which decides whether the
/// `networksetup` fallback is worth trying.
#[derive(Debug)]
pub enum CoreWlanError {
    /// CoreWLAN could not attempt or finish the join (no interface, network not
    /// visible — often missing Location permission — or the link never came
    /// up). The fallback may still succeed.
    Unavailable(String),
    /// The network rejected the join (e.g. wrong password). The fallback would
    /// only repeat the same failure, slowly.
    Rejected(String),
}

/// CoreWLAN security modes that satisfy a QR code's normalized security type
/// ("wpa", "wep" or "nopass"). Joining is refused for any other mode so a
/// same-named network with weaker security (an "evil twin") is never chosen.
#[cfg(target_os = "macos")]
fn accepted_securities(security: &str) -> &'static [CWSecurity] {
    match security {
        "wpa" => &[
            CWSecurity::WPAPersonal,
            CWSecurity::WPAPersonalMixed,
            CWSecurity::WPA2Personal,
            CWSecurity::Personal,
            CWSecurity::WPA3Personal,
            CWSecurity::WPA3Transition,
        ],
        "wep" => &[CWSecurity::WEP],
        _ => &[CWSecurity::None, CWSecurity::OWE, CWSecurity::OWETransition],
    }
}

/// Whether Location Services access has been refused. CoreWLAN needs it both to
/// scan and to read the current SSID, so when it is refused the CoreWLAN path
/// can only waste time.
#[cfg(target_os = "macos")]
pub fn location_denied() -> bool {
    let status = unsafe { CLLocationManager::new().authorizationStatus() };
    status == CLAuthorizationStatus::Denied || status == CLAuthorizationStatus::Restricted
}

/// Join a network via CoreWLAN as the logged-in user — no admin prompt.
/// Requires Location Services permission for the scan. Returns the interface
/// name on success.
///
/// `associateToNetwork` may return `Ok` before the connection is established, so
/// after each associate we poll the interface's current SSID and only report
/// success once it actually matches the target. The scan result is reused
/// across attempts: re-scanning costs seconds and can disrupt a link that is
/// still coming up. Repeated associate errors (e.g. a wrong password) are
/// reported as `Rejected` so the caller does not fall back.
#[cfg(target_os = "macos")]
pub fn join_via_corewlan(
    ssid: &str,
    security: &str,
    password: Option<&str>,
    progress: &dyn Fn(&str),
) -> Result<String, CoreWlanError> {
    unsafe {
        let client = CWWiFiClient::sharedWiFiClient();
        let interface = client
            .interface()
            .ok_or_else(|| CoreWlanError::Unavailable("No Wi-Fi interface available.".into()))?;
        let interface_name = || {
            interface
                .interfaceName()
                .map(|name| name.to_string())
                .unwrap_or_else(|| "Wi-Fi".to_string())
        };

        if current_ssid(&interface).as_deref() == Some(ssid)
            && accepted_securities(security).contains(&interface.security())
        {
            log::info!("Already connected to '{ssid}'");
            return Ok(interface_name());
        }

        progress(&format!("Looking for '{ssid}'..."));
        let mut candidates = find_networks(&interface, ssid, security)?;
        let mut candidate = 0;
        let ns_password = password.map(NSString::from_str);
        let mut auth_failures = 0;
        let mut last_error: Option<(String, isize)> = None;

        for attempt in 1..=COREWLAN_MAX_ATTEMPTS {
            progress(&if attempt == 1 {
                format!("Connecting to '{ssid}'...")
            } else {
                format!("Connecting to '{ssid}' (attempt {attempt} of {COREWLAN_MAX_ATTEMPTS})...")
            });

            let index = candidate % candidates.len();
            let network = &candidates[index];
            let described = describe_network(network);
            log::info!(
                "CoreWLAN attempt {attempt}: access point {} of {} ({described})",
                index + 1,
                candidates.len()
            );

            if let Err(err) =
                interface.associateToNetwork_password_error(network, ns_password.as_deref())
            {
                let code = err.code();
                let detail = nserror_message(&err);
                log::warn!(
                    "CoreWLAN associate attempt {attempt} failed: {} code {code} ({detail}) on {described}",
                    err.domain()
                );
                if is_auth_failure(code) {
                    auth_failures += 1;
                    // Two authentication errors mean the network refused the
                    // key; the fallback would only repeat that, slowly.
                    if auth_failures >= 2 {
                        return Err(CoreWlanError::Rejected(join_failure_message(
                            ssid,
                            &detail,
                            code,
                            password.is_some(),
                        )));
                    }
                }
                last_error = Some((detail, code));

                // A scan result can go stale, and the first copy of the name
                // may be a weak or far access point. Rescan and move on to the
                // next strongest one instead of repeating the same request.
                if attempt < COREWLAN_MAX_ATTEMPTS {
                    progress(&format!("Looking for '{ssid}' again..."));
                    if let Ok(fresh) = find_networks(&interface, ssid, security) {
                        candidates = fresh;
                    }
                    candidate += 1;
                }
                continue;
            }

            if wait_until_connected(&interface, ssid) {
                log::info!("CoreWLAN connected on attempt {attempt}");
                return Ok(interface_name());
            }

            log::info!("CoreWLAN associate attempt {attempt} did not connect; retrying");
        }

        Err(CoreWlanError::Unavailable(match last_error {
            Some((detail, code)) => join_failure_message(ssid, &detail, code, password.is_some()),
            None => format!("Associated with '{ssid}' but the connection did not come up."),
        }))
    }
}

/// Scan for `ssid` and return every access point whose security matches
/// `security`, strongest signal first. Retries the scan once because a scan
/// can miss a network that is in range (or fail transiently while the radio is
/// busy).
///
/// One network name is usually several access points (2.4 and 5 GHz radios,
/// mesh nodes). Taking whichever one the scan lists first can pick a weak or
/// far radio that refuses the join, while the Wi-Fi menu picks the best one.
///
/// Anyone can broadcast any SSID, so the name alone does not identify the
/// network the QR code describes. If the only networks with that name use a
/// different security type, the join is refused as `Rejected` so the
/// networksetup fallback (which cannot check security) does not join it either.
#[cfg(target_os = "macos")]
fn find_networks(
    interface: &CWInterface,
    ssid: &str,
    security: &str,
) -> Result<Vec<objc2::rc::Retained<CWNetwork>>, CoreWlanError> {
    let ns_ssid = NSString::from_str(ssid);
    let accepted = accepted_securities(security);
    let mut last_error = format!("Network '{ssid}' was not found in range.");
    let mut saw_mismatch = false;

    for _ in 0..2 {
        match unsafe { interface.scanForNetworksWithName_includeHidden_error(Some(&ns_ssid), true) }
        {
            Ok(networks) => {
                let mut matching = Vec::new();
                for network in networks.iter() {
                    if accepted
                        .iter()
                        .any(|mode| unsafe { network.supportsSecurity(*mode) })
                    {
                        let rssi = unsafe { network.rssiValue() };
                        matching.push((network, rssi));
                    } else {
                        saw_mismatch = true;
                    }
                }
                if !matching.is_empty() {
                    log::info!("Scan found {} matching access point(s)", matching.len());
                    return Ok(strongest_first(matching));
                }
            }
            Err(err) => last_error = nserror_message(&err),
        }
    }

    if saw_mismatch {
        return Err(CoreWlanError::Rejected(format!(
            "Found a network named '{ssid}', but it does not use the security the QR code declares, so WiLens did not join it."
        )));
    }
    Err(CoreWlanError::Unavailable(last_error))
}

/// Channel, band and signal of a scanned access point, for the log. Leaves out
/// the BSSID and the network name.
#[cfg(target_os = "macos")]
fn describe_network(network: &CWNetwork) -> String {
    use objc2_core_wlan::CWChannelBand;
    unsafe {
        let channel = match network.wlanChannel() {
            Some(channel) => {
                let band = match channel.channelBand() {
                    CWChannelBand::Band2GHz => "2.4 GHz",
                    CWChannelBand::Band5GHz => "5 GHz",
                    CWChannelBand::Band6GHz => "6 GHz",
                    _ => "unknown band",
                };
                format!("channel {} {band}", channel.channelNumber())
            }
            None => "unknown channel".to_string(),
        };
        format!("{channel}, {} dBm", network.rssiValue())
    }
}

/// Order scanned access points strongest signal first (RSSI is in dBm, so
/// closer to zero is stronger). Equal signals keep the scan's order.
#[cfg(any(target_os = "macos", test))]
fn strongest_first<T>(mut networks: Vec<(T, isize)>) -> Vec<T> {
    networks.sort_by_key(|(_, rssi)| std::cmp::Reverse(*rssi));
    networks.into_iter().map(|(network, _)| network).collect()
}

/// CoreWLAN error codes that mean the network refused the key: an EAPOL
/// (4-way handshake) failure, a bad PMK, a failed challenge, or the supplicant
/// timing out during the handshake. Other codes, such as -3900 ("tmpErr") or
/// -3906, are general failures and say nothing about the password.
#[cfg(any(target_os = "macos", test))]
fn is_auth_failure(code: isize) -> bool {
    matches!(code, 1 | -3912 | -3924 | -3925)
}

/// User-facing message for a failed CoreWLAN join. Only an authentication
/// error suggests checking the password.
#[cfg(any(target_os = "macos", test))]
fn join_failure_message(ssid: &str, detail: &str, code: isize, has_password: bool) -> String {
    if has_password && is_auth_failure(code) {
        format!(
            "Could not join '{ssid}': the network did not accept the password (error {code}). Check that the password is correct."
        )
    } else {
        let detail = detail.trim_end_matches('.');
        format!(
            "Could not join '{ssid}': {detail} (error {code}). Try again closer to the router, or copy the password and join from the Wi-Fi menu."
        )
    }
}

/// Poll the interface's current SSID until it matches `ssid` or the timeout
/// elapses. Returns `true` once connected.
#[cfg(target_os = "macos")]
fn wait_until_connected(interface: &CWInterface, ssid: &str) -> bool {
    let deadline = Instant::now() + COREWLAN_VERIFY_TIMEOUT;
    loop {
        if current_ssid(interface).as_deref() == Some(ssid) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(COREWLAN_VERIFY_INTERVAL);
    }
}

/// The SSID the interface is currently associated with, if any. Returns `None`
/// without Location permission even when connected.
#[cfg(target_os = "macos")]
fn current_ssid(interface: &CWInterface) -> Option<String> {
    unsafe { interface.ssid().map(|name| name.to_string()) }
}

/// The SSID of the default Wi-Fi interface, if CoreWLAN can report it.
#[cfg(target_os = "macos")]
pub fn default_interface_ssid() -> Option<String> {
    unsafe {
        let interface = CWWiFiClient::sharedWiFiClient().interface()?;
        current_ssid(&interface)
    }
}

#[cfg(target_os = "macos")]
fn nserror_message(error: &NSError) -> String {
    // NSError implements Display in objc2-foundation (via localizedDescription).
    error.to_string()
}

/// Join an open network using the legacy `networksetup` command. Returns the
/// interface name on success. Secured networks never come here: networksetup
/// would need the password on its command line and cannot check security.
pub fn join_via_networksetup(ssid: &str) -> Result<String, String> {
    let interface = detect_wifi_interface()?;

    let output = run_with_timeout(
        Command::new("/usr/sbin/networksetup").args(["-setairportnetwork", &interface, ssid]),
        NETWORKSETUP_TIMEOUT,
        "networksetup",
    )?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() { stderr } else { stdout };

    // `-setairportnetwork` exits 0 even when the join fails; the failure is
    // only reported as text ("Could not find network ...", "Failed to join
    // network ...", "Error: -3900 ...").
    if !output.status.success() || reports_failure(&details) {
        return Err(if details.is_empty() {
            "networksetup failed without an error message.".to_string()
        } else {
            details
        });
    }

    Ok(interface)
}

fn reports_failure(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    ["could not", "failed", "error"]
        .iter()
        .any(|needle| lower.contains(needle))
}

pub fn detect_wifi_interface() -> Result<String, String> {
    let output = run_with_timeout(
        Command::new("/usr/sbin/networksetup").arg("-listallhardwareports"),
        NETWORKSETUP_TIMEOUT,
        "networksetup",
    )?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let ports = parse_hardware_ports(&String::from_utf8_lossy(&output.stdout));

    ports
        .into_iter()
        .find(|entry| entry.port.contains("Wi-Fi") || entry.port.contains("AirPort"))
        .map(|entry| entry.device)
        .ok_or_else(|| "Unable to find the Wi-Fi interface on this Mac.".to_string())
}

pub fn parse_hardware_ports(output: &str) -> Vec<HardwarePort> {
    let mut ports = Vec::new();
    let mut current_port: Option<String> = None;
    let mut current_device: Option<String> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if let (Some(port), Some(device)) = (current_port.take(), current_device.take()) {
                ports.push(HardwarePort { port, device });
            }
            continue;
        }

        if let Some(port) = trimmed.strip_prefix("Hardware Port: ") {
            current_port = Some(port.trim().to_string());
            continue;
        }

        if let Some(device) = trimmed.strip_prefix("Device: ") {
            current_device = Some(device.trim().to_string());
        }
    }

    if let (Some(port), Some(device)) = (current_port, current_device) {
        ports.push(HardwarePort { port, device });
    }

    ports
}

#[cfg(test)]
mod tests {
    use super::{
        is_auth_failure, join_failure_message, parse_hardware_ports, reports_failure,
        strongest_first,
    };

    #[test]
    fn orders_access_points_strongest_first() {
        let ordered = strongest_first(vec![
            ("far", -80),
            ("near", -40),
            ("mid", -60),
            ("near2", -40),
        ]);
        assert_eq!(ordered, vec!["near", "near2", "mid", "far"]);
    }

    #[test]
    fn only_authentication_errors_blame_the_password() {
        assert!(is_auth_failure(1));
        assert!(is_auth_failure(-3924));
        assert!(!is_auth_failure(-3900));
        assert!(!is_auth_failure(-3906));

        let general = join_failure_message(
            "608",
            "The operation couldn't be completed. tmpErr.",
            -3900,
            true,
        );
        assert!(!general.contains("password is correct"), "{general}");
        assert!(general.contains("tmpErr (error -3900)"), "{general}");

        let auth = join_failure_message("608", "EAPOL", 1, true);
        assert!(
            auth.contains("Check that the password is correct"),
            "{auth}"
        );

        let open = join_failure_message("608", "EAPOL", 1, false);
        assert!(!open.contains("password is correct"), "{open}");
    }

    #[test]
    fn detects_networksetup_failure_text() {
        assert!(reports_failure("Could not find network MyWifi."));
        assert!(reports_failure(
            "Failed to join network MyWifi.\nError: -3900  The operation couldn't be completed."
        ));
        assert!(!reports_failure(""));
    }

    #[test]
    fn parses_networksetup_hardware_ports() {
        let ports = parse_hardware_ports(
      "Hardware Port: Wi-Fi\nDevice: en0\nEthernet Address: aa:bb:cc:dd:ee:ff\n\nHardware Port: Bluetooth PAN\nDevice: en7\n",
    );

        assert_eq!(ports.len(), 2);
        assert_eq!(ports[0].port, "Wi-Fi");
        assert_eq!(ports[0].device, "en0");
        assert_eq!(ports[1].port, "Bluetooth PAN");
        assert_eq!(ports[1].device, "en7");
    }
}
