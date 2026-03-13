use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JoinWifiRequest {
    ssid: String,
    password: Option<String>,
    security: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinWifiResponse {
    interface: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    message: String,
}

#[derive(Debug)]
struct HardwarePort {
    port: String,
    device: String,
}

#[tauri::command]
fn join_wifi(request: JoinWifiRequest) -> Result<JoinWifiResponse, ErrorResponse> {
    let ssid = sanitize_required(&request.ssid, "SSID")?;
    let security = normalize_security(&request.security)?;
    let password = sanitize_optional(request.password.as_deref(), "Password")?;
    let interface = detect_wifi_interface()?;

    let mut args = vec![
        "-setairportnetwork".to_string(),
        interface.clone(),
        ssid.clone(),
    ];

    if security != "nopass" {
        let value = password.ok_or_else(|| ErrorResponse {
            message: "Password is required for secured networks.".into(),
        })?;

        args.push(value);
    }

    let output = Command::new("/usr/sbin/networksetup")
        .args(&args)
        .output()
        .map_err(|error| ErrorResponse {
            message: format!("Failed to run networksetup: {error}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };

        return Err(ErrorResponse {
            message: if details.is_empty() {
                "networksetup failed without an error message.".into()
            } else {
                details
            },
        });
    }

    Ok(JoinWifiResponse {
        interface,
        message: format!("Joined '{ssid}' successfully."),
    })
}

fn sanitize_required(value: &str, label: &str) -> Result<String, ErrorResponse> {
    if value.trim().is_empty() {
        return Err(ErrorResponse {
            message: format!("{label} is required."),
        });
    }

    if value
        .chars()
        .any(|char| char == '\0' || char == '\n' || char == '\r')
    {
        return Err(ErrorResponse {
            message: format!("{label} contains invalid control characters."),
        });
    }

    Ok(value.to_string())
}

fn sanitize_optional(value: Option<&str>, label: &str) -> Result<Option<String>, ErrorResponse> {
    match value {
        Some(raw) if !raw.is_empty() => sanitize_required(raw, label).map(Some),
        _ => Ok(None),
    }
}

fn normalize_security(value: &str) -> Result<String, ErrorResponse> {
    let normalized = value.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "wpa" | "wep" | "nopass" => Ok(normalized),
        _ => Err(ErrorResponse {
            message: format!("Unsupported security type '{value}'."),
        }),
    }
}

fn detect_wifi_interface() -> Result<String, ErrorResponse> {
    let output = Command::new("/usr/sbin/networksetup")
        .arg("-listallhardwareports")
        .output()
        .map_err(|error| ErrorResponse {
            message: format!("Failed to inspect hardware ports: {error}"),
        })?;

    if !output.status.success() {
        return Err(ErrorResponse {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let ports = parse_hardware_ports(&String::from_utf8_lossy(&output.stdout));

    ports
        .into_iter()
        .find(|entry| entry.port.contains("Wi-Fi") || entry.port.contains("AirPort"))
        .map(|entry| entry.device)
        .ok_or_else(|| ErrorResponse {
            message: "Unable to find the Wi-Fi interface on this Mac.".into(),
        })
}

fn parse_hardware_ports(output: &str) -> Vec<HardwarePort> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(tauri::generate_handler![join_wifi])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
