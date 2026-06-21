use std::process::Command;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
use objc2_core_wlan::{CWInterface, CWWiFiClient};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSError, NSString};

/// Maximum number of scan + associate attempts before giving up on CoreWLAN.
/// `associateToNetwork` can report success before the link is actually up; the
/// second attempt is what reliably connects (the behavior users hit manually as
/// a "double click"), so we retry automatically.
#[cfg(target_os = "macos")]
const COREWLAN_MAX_ATTEMPTS: u32 = 3;

/// How long to wait for the interface to report the target SSID after an
/// associate call returns, before treating the attempt as not-yet-connected.
#[cfg(target_os = "macos")]
const COREWLAN_VERIFY_TIMEOUT: Duration = Duration::from_millis(2500);

/// Polling interval while verifying the connection.
#[cfg(target_os = "macos")]
const COREWLAN_VERIFY_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug)]
pub struct HardwarePort {
    pub port: String,
    pub device: String,
}

/// Join a network via CoreWLAN as the logged-in user — no admin prompt.
/// Requires Location Services permission for the scan. Returns the interface
/// name on success.
///
/// `associateToNetwork` may return `Ok` before the connection is established, so
/// after each associate we poll the interface's current SSID and only report
/// success once it actually matches the target. If it does not connect within
/// the timeout we re-scan and try again, up to `COREWLAN_MAX_ATTEMPTS`.
#[cfg(target_os = "macos")]
pub fn join_via_corewlan(ssid: &str, password: Option<&str>) -> Result<String, String> {
    unsafe {
        let client = CWWiFiClient::sharedWiFiClient();
        let interface = client
            .interface()
            .ok_or_else(|| "No Wi-Fi interface available.".to_string())?;

        let ns_ssid = NSString::from_str(ssid);
        let ns_password = password.map(NSString::from_str);
        let mut last_error: Option<String> = None;

        for attempt in 1..=COREWLAN_MAX_ATTEMPTS {
            let networks = match interface
                .scanForNetworksWithName_includeHidden_error(Some(&ns_ssid), true)
            {
                Ok(networks) => networks,
                Err(err) => {
                    last_error = Some(nserror_message(&err));
                    continue;
                }
            };

            let Some(network) = networks.anyObject() else {
                last_error = Some(format!("Network '{ssid}' was not found in range."));
                continue;
            };

            if let Err(err) =
                interface.associateToNetwork_password_error(&network, ns_password.as_deref())
            {
                last_error = Some(nserror_message(&err));
                log::info!("CoreWLAN associate attempt {attempt} failed; retrying");
                continue;
            }

            if wait_until_connected(&interface, ssid) {
                log::info!("CoreWLAN connected on attempt {attempt}");
                return Ok(interface
                    .interfaceName()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| "Wi-Fi".to_string()));
            }

            last_error = Some(format!(
                "Associated with '{ssid}' but the connection did not come up."
            ));
            log::info!("CoreWLAN associate attempt {attempt} did not connect; retrying");
        }

        Err(last_error
            .unwrap_or_else(|| format!("Could not connect to '{ssid}' via CoreWLAN.")))
    }
}

/// Poll the interface's current SSID until it matches `ssid` or the timeout
/// elapses. Returns `true` once connected.
#[cfg(target_os = "macos")]
fn wait_until_connected(interface: &CWInterface, ssid: &str) -> bool {
    let mut waited = Duration::ZERO;
    loop {
        if current_ssid(interface).as_deref() == Some(ssid) {
            return true;
        }
        if waited >= COREWLAN_VERIFY_TIMEOUT {
            return false;
        }
        std::thread::sleep(COREWLAN_VERIFY_INTERVAL);
        waited += COREWLAN_VERIFY_INTERVAL;
    }
}

/// The SSID the interface is currently associated with, if any.
#[cfg(target_os = "macos")]
fn current_ssid(interface: &CWInterface) -> Option<String> {
    unsafe { interface.ssid().map(|name| name.to_string()) }
}

#[cfg(target_os = "macos")]
fn nserror_message(error: &NSError) -> String {
    // NSError implements Display in objc2-foundation (via localizedDescription).
    error.to_string()
}

/// Join a network using the legacy `networksetup` command. Requires admin
/// authorization (macOS shows a prompt). Returns the interface name on success.
pub fn join_via_networksetup(
    ssid: &str,
    security: &str,
    password: Option<&str>,
) -> Result<String, String> {
    let interface = detect_wifi_interface()?;

    let mut args = vec![
        "-setairportnetwork".to_string(),
        interface.clone(),
        ssid.to_string(),
    ];

    if security != "nopass" {
        let value =
            password.ok_or_else(|| "Password is required for secured networks.".to_string())?;
        args.push(value.to_string());
    }

    let output = Command::new("/usr/sbin/networksetup")
        .args(&args)
        .output()
        .map_err(|error| format!("Failed to run networksetup: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if details.is_empty() {
            "networksetup failed without an error message.".to_string()
        } else {
            details
        });
    }

    Ok(interface)
}

pub fn detect_wifi_interface() -> Result<String, String> {
    let output = Command::new("/usr/sbin/networksetup")
        .arg("-listallhardwareports")
        .output()
        .map_err(|error| format!("Failed to inspect hardware ports: {error}"))?;

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
    use super::parse_hardware_ports;

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
