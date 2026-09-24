use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use wifi::CoreWlanError;

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
async fn join_wifi(
    request: JoinWifiRequest,
    on_progress: Channel<String>,
) -> Result<JoinWifiResponse, ErrorResponse> {
    // CoreWLAN scans, association, verification polling, and the fallback
    // process invocation can all block for seconds. Run them on Tauri's
    // dedicated blocking pool so the macOS event loop keeps servicing the
    // window instead of showing the spinning beach ball.
    tauri::async_runtime::spawn_blocking(move || {
        let progress = |message: &str| {
            let _ = on_progress.send(message.to_string());
        };
        join_wifi_blocking(request, &progress)
    })
    .await
    .map_err(|error| ErrorResponse {
        message: format!("Wi-Fi join task ended unexpectedly: {error}"),
    })?
}

fn join_wifi_blocking(
    request: JoinWifiRequest,
    progress: &dyn Fn(&str),
) -> Result<JoinWifiResponse, ErrorResponse> {
    let ssid = sanitize_required(&request.ssid, "SSID")?;
    let security = normalize_security(&request.security)?;
    let password = sanitize_optional(request.password.as_deref(), "Password")?;

    let corewlan_result = if wifi::location_denied() {
        Err(CoreWlanError::Unavailable(
            "Location access is denied, so CoreWLAN cannot scan.".to_string(),
        ))
    } else {
        wifi::join_via_corewlan(&ssid, password.as_deref(), progress)
    };

    let (interface, method) = match corewlan_result {
        Ok(interface) => (interface, "corewlan"),
        // The network itself refused the join; networksetup would only repeat
        // the same failure after another long wait.
        Err(CoreWlanError::Rejected(message)) => return Err(ErrorResponse { message }),
        Err(CoreWlanError::Unavailable(corewlan_error)) => {
            log::warn!("CoreWLAN join failed, falling back to networksetup: {corewlan_error}");
            ensure_not_flag(&ssid, "SSID")?;
            if let Some(password) = password.as_deref() {
                ensure_not_flag(password, "Password")?;
            }
            progress("Trying the system fallback...");
            let interface = wifi::join_via_networksetup(&ssid, &security, password.as_deref())
                .map_err(|message| ErrorResponse { message })?;
            // The SSID is unreadable without Location access; only a mismatch
            // is conclusive.
            if let Some(current) = wifi::default_interface_ssid() {
                if current != ssid {
                    return Err(ErrorResponse {
                        message: format!(
                            "Could not join '{ssid}'; still connected to '{current}'."
                        ),
                    });
                }
            }
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

    Ok(value.to_string())
}

/// networksetup receives values as positional arguments. A value that begins
/// with '-' would be misread as a command-line flag (argument injection), so
/// the fallback refuses it. CoreWLAN takes values directly and needs no check.
fn ensure_not_flag(value: &str, label: &str) -> Result<(), ErrorResponse> {
    if value.starts_with('-') {
        return Err(ErrorResponse {
            message: format!(
                "Could not join with CoreWLAN, and the fallback cannot use a {label} starting with '-'."
            ),
        });
    }
    Ok(())
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
    use super::{ensure_not_flag, normalize_security, sanitize_optional, sanitize_required};

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
    fn sanitize_required_accepts_leading_dash() {
        // Valid for CoreWLAN; only the networksetup fallback refuses it.
        assert_eq!(sanitize_required("-pass", "Password").unwrap(), "-pass");
    }

    #[test]
    fn ensure_not_flag_rejects_leading_dash() {
        assert!(ensure_not_flag("-setairportpower", "SSID").is_err());
        assert!(ensure_not_flag("MyWifi", "SSID").is_ok());
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
