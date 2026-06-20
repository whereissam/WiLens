use std::process::Command;

#[derive(Debug)]
pub struct HardwarePort {
    pub port: String,
    pub device: String,
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
