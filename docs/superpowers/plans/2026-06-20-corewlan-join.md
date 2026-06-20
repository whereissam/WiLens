# CoreWLAN Wi-Fi Join Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Join Wi-Fi via CoreWLAN (no admin prompt) with an automatic `networksetup` fallback, requesting Location permission at startup.

**Architecture:** The `join_wifi` Tauri command keeps its existing input validation, then delegates the actual join to a new `wifi` module. It tries `join_via_corewlan` first; on any error it falls back to the existing `networksetup` logic. The response reports which method ran. Location authorization is requested once at app startup so CoreWLAN scanning works silently.

**Tech Stack:** Rust, Tauri 2, `objc2` 0.6, `objc2-foundation` 0.3, `objc2-core-wlan` 0.3, `objc2-core-location` 0.3.

## Global Constraints

- Platform: macOS only. The `objc2-*` dependencies must be gated under `[target.'cfg(target_os = "macos")'.dependencies]`.
- Rust edition 2021, `rust-version = 1.77.2` (from `src-tauri/Cargo.toml` — do not lower).
- `cargo clippy --all-targets -- -D warnings` must stay clean (CI gate).
- Existing unit tests must stay green: `sanitize_required`, `sanitize_optional`, `normalize_security`, `parse_hardware_ports`.
- Do not log raw passwords (existing project rule, `docs/todo.md` Phase 6).
- All `objc2` framework methods are `unsafe`; every call site wraps them in `unsafe { }`.

---

### Task 1: Refactor join into a `wifi` module (no behavior change)

Move the `networksetup` join + interface detection out of `lib.rs` into a new `wifi` module, add a `method` field to the response, and gate the `objc2-*` deps to macOS. This is a pure refactor verified by the existing tests.

**Files:**
- Create: `src-tauri/src/wifi.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `wifi::join_via_networksetup(ssid: &str, security: &str, password: Option<&str>) -> Result<String, String>` (returns interface name on success, error message string on failure).
- Produces: `wifi::detect_wifi_interface() -> Result<String, String>`, `wifi::parse_hardware_ports(output: &str) -> Vec<wifi::HardwarePort>`.
- Consumes (in `lib.rs`): the above, plus the unchanged `JoinWifiRequest` / `JoinWifiResponse` / `ErrorResponse` structs.

- [ ] **Step 1: Gate objc2 deps to macOS in `Cargo.toml`**

Replace the four `objc2*` lines currently under `[dependencies]` with a target-gated section. The `[dependencies]` block should end at `tauri-plugin-log`, then add:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-foundation = "0.3"
objc2-core-wlan = { version = "0.3", features = ["CWWiFiClient", "CWInterface", "CWNetwork"] }
objc2-core-location = { version = "0.3", features = ["CLLocationManager"] }
```

- [ ] **Step 2: Create `src-tauri/src/wifi.rs` with the moved networksetup logic**

```rust
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
        let value = password.ok_or_else(|| "Password is required for secured networks.".to_string())?;
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
```

- [ ] **Step 3: Update `lib.rs` — declare the module, remove moved code, add `method` field, call the new function**

At the top of `lib.rs`, after the `use` lines, add:

```rust
mod wifi;
```

Delete from `lib.rs`: the `HardwarePort` struct, `detect_wifi_interface`, `parse_hardware_ports`, and the `#[cfg(test)]` module containing `parses_networksetup_hardware_ports` (all now live in `wifi.rs`). Keep `sanitize_required`, `sanitize_optional`, `normalize_security`.

Add `method` to the response struct:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinWifiResponse {
    interface: String,
    message: String,
    method: String,
}
```

Replace the body of `join_wifi` (the part after the three `let ssid/security/password` validation lines) with a call into the module:

```rust
#[tauri::command]
fn join_wifi(request: JoinWifiRequest) -> Result<JoinWifiResponse, ErrorResponse> {
    let ssid = sanitize_required(&request.ssid, "SSID")?;
    let security = normalize_security(&request.security)?;
    let password = sanitize_optional(request.password.as_deref(), "Password")?;

    let interface = wifi::join_via_networksetup(&ssid, &security, password.as_deref())
        .map_err(|message| ErrorResponse { message })?;

    Ok(JoinWifiResponse {
        interface,
        message: format!("Joined '{ssid}' successfully."),
        method: "networksetup".to_string(),
    })
}
```

- [ ] **Step 4: Verify build, lint, and tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS, including `wifi::tests::parses_networksetup_hardware_ports` and the three sanitize/normalize tests.

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/wifi.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "refactor: move Wi-Fi join into wifi module, add method field"
```

---

### Task 2: Implement the CoreWLAN join path with fallback

Add `join_via_corewlan` to `wifi.rs` and wire `join_wifi` to try it first, falling back to `networksetup`.

**Files:**
- Modify: `src-tauri/src/wifi.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `wifi::join_via_corewlan(ssid: &str, password: Option<&str>) -> Result<String, String>` (returns interface name on success).
- Consumes: `wifi::join_via_networksetup` (Task 1).

- [ ] **Step 1: Add the CoreWLAN join function to `wifi.rs`**

Add at the top of `wifi.rs`:

```rust
#[cfg(target_os = "macos")]
use objc2_core_wlan::CWWiFiClient;
#[cfg(target_os = "macos")]
use objc2_foundation::{NSError, NSString};
```

Add the function (signatures verified against `objc2-core-wlan` 0.3.2):

```rust
/// Join a network via CoreWLAN as the logged-in user — no admin prompt.
/// Requires Location Services permission for the scan. Returns the interface
/// name on success.
#[cfg(target_os = "macos")]
pub fn join_via_corewlan(ssid: &str, password: Option<&str>) -> Result<String, String> {
    unsafe {
        let client = CWWiFiClient::sharedWiFiClient();
        let interface = client
            .interface()
            .ok_or_else(|| "No Wi-Fi interface available.".to_string())?;

        let ns_ssid = NSString::from_str(ssid);
        let networks = interface
            .scanForNetworksWithName_includeHidden_error(Some(&ns_ssid), true)
            .map_err(|err| nserror_message(&err))?;

        let network = networks
            .anyObject()
            .ok_or_else(|| format!("Network '{ssid}' was not found in range."))?;

        let ns_password = password.map(NSString::from_str);
        interface
            .associateToNetwork_password_error(&network, ns_password.as_deref())
            .map_err(|err| nserror_message(&err))?;

        Ok(interface
            .interfaceName()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "Wi-Fi".to_string()))
    }
}

#[cfg(target_os = "macos")]
fn nserror_message(error: &NSError) -> String {
    // NSError implements Display in objc2-foundation (via localizedDescription).
    error.to_string()
}
```

- [ ] **Step 2: Wire `join_wifi` to try CoreWLAN first, then fall back**

In `lib.rs`, replace the `join_wifi` body's join section (the `let interface = wifi::join_via_networksetup(...)` block and the `Ok(JoinWifiResponse { ... })`) with:

```rust
    let (interface, method) = match wifi::join_via_corewlan(&ssid, password.as_deref()) {
        Ok(interface) => (interface, "corewlan"),
        Err(corewlan_error) => {
            log::warn!("CoreWLAN join failed, falling back to networksetup: {corewlan_error}");
            let interface = wifi::join_via_networksetup(&ssid, &security, password.as_deref())
                .map_err(|message| ErrorResponse { message })?;
            (interface, "networksetup")
        }
    };

    Ok(JoinWifiResponse {
        interface,
        message: format!("Joined '{ssid}' successfully."),
        method: method.to_string(),
    })
```

Note: do not log `password` — only the error string and method are logged.

- [ ] **Step 3: Verify build and lint**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles. (First build pulls the objc2 crates; allow time.)

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: no warnings.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS (unchanged unit tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/wifi.rs src-tauri/src/lib.rs
git commit -m "feat: join Wi-Fi via CoreWLAN with networksetup fallback"
```

---

### Task 3: Request Location permission at startup

Add the Info.plist usage string and request Location authorization in the Tauri `setup` hook so CoreWLAN scanning works silently.

**Files:**
- Modify: `src-tauri/Info.plist`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `request_location_authorization()` (macOS-only, called from `setup`).

- [ ] **Step 1: Add the Location usage description to `Info.plist`**

Inside the `<dict>`, after the existing `NSCameraUsageDescription` key/string pair, add:

```xml
  <key>NSLocationWhenInUseUsageDescription</key>
  <string>WiLens needs your location because macOS requires it to scan for and join Wi-Fi networks.</string>
```

- [ ] **Step 2: Add the authorization request function to `lib.rs`**

Add near the bottom of `lib.rs`, before `run()`:

```rust
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
```

- [ ] **Step 3: Call it from the Tauri `setup` hook**

In `run()`, add a `.setup(...)` call to the builder chain (before `.invoke_handler(...)`):

```rust
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            request_location_authorization();
            Ok(())
        })
```

- [ ] **Step 4: Verify build and lint**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles.

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Info.plist src-tauri/src/lib.rs
git commit -m "feat: request Location permission at startup for CoreWLAN scan"
```

---

### Task 4: End-to-end verification and docs

Build the full app, verify behavior manually, and update the todo.

**Files:**
- Modify: `docs/todo.md`

- [ ] **Step 1: Build the full app bundle**

Run: `cargo tauri build`
Expected: builds `WiLens.app` and the `.dmg` with no errors.

- [ ] **Step 2: Manual verification (record results)**

1. Launch the app → confirm the **Location** permission prompt appears at startup (with the Info.plist wording). Grant it.
2. Scan a known Wi-Fi QR code and Join → confirm **no admin/keychain prompt** appears and the network connects.
3. Confirm the response `method` is `corewlan` (check the app log via `tauri-plugin-log`, or temporarily surface `method` in the status message).
4. Fallback check: in System Settings deny Location for WiLens, relaunch, and join → confirm it still connects via `networksetup` (admin prompt expected) and the log shows the fallback warning.

> **Risk note:** On an **unsigned/ad-hoc** build, macOS may not persist the Location grant reliably. If the prompt does not appear or scanning returns nothing, the app correctly falls back to `networksetup`. The CoreWLAN benefit is fully realized once the app is code-signed; the code is correct regardless.

- [ ] **Step 3: Update `docs/todo.md`**

Under `### 7b. Reliability`, mark the networksetup line done and add the CoreWLAN line:

```markdown
- [x] Verify `networksetup -setairportnetwork` works on the target macOS version. *(kept as fallback; primary join now uses CoreWLAN)*
- [x] Document the `Copy password` manual fallback as the supported Plan B if the join command fails. *(already in `docs/usage.md`)*
- [x] Replace the per-join admin prompt by joining via CoreWLAN; request Location permission at startup; fall back to networksetup. *(see `docs/superpowers/specs/2026-06-20-corewlan-join-design.md`)*
```

- [ ] **Step 4: Commit**

```bash
git add docs/todo.md
git commit -m "docs: mark CoreWLAN join done in todo"
```
