use serde::{Deserialize, Serialize};

mod wifi;

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
    method: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    message: String,
}

#[tauri::command]
fn join_wifi(request: JoinWifiRequest) -> Result<JoinWifiResponse, ErrorResponse> {
    let ssid = sanitize_required(&request.ssid, "SSID")?;
    let security = normalize_security(&request.security)?;
    let password = sanitize_optional(request.password.as_deref(), "Password")?;

    let (interface, method) = match wifi::join_via_corewlan(&ssid, password.as_deref()) {
        Ok(interface) => (interface, "corewlan"),
        Err(corewlan_error) => {
            log::warn!("CoreWLAN join failed, falling back to networksetup: {corewlan_error}");
            let interface = wifi::join_via_networksetup(&ssid, &security, password.as_deref())
                .map_err(|message| ErrorResponse { message })?;
            (interface, "networksetup")
        }
    };
    log::info!("Joined '{ssid}' via {method} on {interface}");

    Ok(JoinWifiResponse {
        interface,
        message: format!("Joined '{ssid}' successfully."),
        method: method.to_string(),
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

    // networksetup receives these values as positional arguments. A value that
    // begins with '-' would be misread as a command-line flag (argument
    // injection), so reject it rather than risk an unexpected invocation.
    if value.starts_with('-') {
        return Err(ErrorResponse {
            message: format!("{label} must not start with '-'."),
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

#[cfg(target_os = "macos")]
fn request_location_authorization() {
    use objc2_core_location::CLLocationManager;
    unsafe {
        let manager = CLLocationManager::new();
        manager.requestWhenInUseAuthorization();
        // Keep the manager alive for the app's lifetime so the asynchronous
        // authorization prompt can complete.
        std::mem::forget(manager);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            request_location_authorization();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![join_wifi])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{normalize_security, sanitize_optional, sanitize_required};

    #[test]
    fn sanitize_required_accepts_a_normal_value() {
        assert_eq!(sanitize_required("MyWifi", "SSID").unwrap(), "MyWifi");
    }

    #[test]
    fn sanitize_required_rejects_empty_and_whitespace() {
        assert!(sanitize_required("", "SSID").is_err());
        assert!(sanitize_required("   ", "SSID").is_err());
    }

    #[test]
    fn sanitize_required_rejects_control_characters() {
        assert!(sanitize_required("My\nWifi", "SSID").is_err());
        assert!(sanitize_required("My\0Wifi", "SSID").is_err());
        assert!(sanitize_required("My\rWifi", "SSID").is_err());
    }

    #[test]
    fn sanitize_required_rejects_leading_dash() {
        assert!(sanitize_required("-setairportpower", "SSID").is_err());
    }

    #[test]
    fn sanitize_optional_maps_present_and_absent_values() {
        assert_eq!(
            sanitize_optional(Some("pw"), "Password").unwrap(),
            Some("pw".to_string())
        );
        assert_eq!(sanitize_optional(Some(""), "Password").unwrap(), None);
        assert_eq!(sanitize_optional(None, "Password").unwrap(), None);
    }

    #[test]
    fn normalize_security_accepts_supported_types() {
        assert_eq!(normalize_security("WPA").unwrap(), "wpa");
        assert_eq!(normalize_security("wep").unwrap(), "wep");
        assert_eq!(normalize_security(" nopass ").unwrap(), "nopass");
    }

    #[test]
    fn normalize_security_rejects_unsupported_types() {
        assert!(normalize_security("wpa3-enterprise").is_err());
        assert!(normalize_security("").is_err());
    }
}
