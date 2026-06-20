# CoreWLAN Wi-Fi Join — Design

**Date:** 2026-06-20
**Status:** Approved, pending implementation plan

## Problem

Joining a network currently shells out to `networksetup -setairportnetwork`. That
command writes the Wi-Fi password into the **System keychain** and changes
system-wide network configuration, so macOS requires an **administrator name +
password** authorization dialog **on every join**. This is the app's biggest UX
wart: a one-line QR scan is followed by a heavyweight admin prompt each time.

## Goal

Eliminate the admin prompt for the common case by joining through Apple's
**CoreWLAN** framework, which associates to a network as the logged-in user
without touching the System keychain. The trade-off (accepted by the user) is a
**one-time Location Services permission**, because since macOS Sonoma Wi-Fi
scanning requires location authorization.

Non-goal: zero prompts. CoreWLAN swaps "admin prompt every join" for "location
prompt once."

## Approach

### Flow

The existing `join_wifi` Tauri command and all current input validation
(`sanitize_required`, `sanitize_optional`, `normalize_security`) are kept
unchanged — they are already unit-tested. Only the actual join is changed, split
into two strategies with automatic fallback:

```
join_wifi (validate input)
   └─> try join_via_corewlan(ssid, password, security)
         ├─ success ─> return { method: "corewlan", interface, message }
         └─ any error ─> join_via_networksetup(...)   // today's code, unchanged
                           ├─ success ─> return { method: "networksetup", ... }
                           └─ error ─> return error
```

The fallback guarantees a network can always be joined; only the rare edge cases
(see below) still show the admin prompt.

### CoreWLAN join path

1. Obtain the default Wi-Fi interface via `CWWiFiClient.sharedWiFiClient().interface()`.
2. `scanForNetworks(withName: ssid)` to locate the `CWNetwork`. The `withName`
   form also finds **hidden** networks when the SSID is supplied.
3. `associate(to: network, password:)` for secured networks, or the
   password-less associate for open (`nopass`) networks.
4. Any failure (no interface, scan empty, association error) returns an error so
   `join_wifi` falls back to `networksetup`.

### Location permission

- **Request at app startup** in the Tauri `setup` hook via CoreLocation
  (`CLLocationManager` → `requestWhenInUseAuthorization`). The system prompt
  appears when the app first opens, decoupled from scanning, so by the time the
  user scans a QR the permission is usually already granted and CoreWLAN runs
  silently.
- **Add `NSLocationWhenInUseUsageDescription`** to `src-tauri/Info.plist`:
  > "WiLens needs your location because macOS requires it to scan for and join
  > Wi-Fi networks."
- **Graceful degradation:** if location is still `notDetermined`/`denied` at join
  time, the CoreWLAN scan returns nothing and the app falls back to
  `networksetup` (admin prompt). The app never breaks; worst case equals today's
  behavior. The first-ever join may fall back if permission has not yet been
  granted.

## Components

- `src-tauri/src/wifi.rs` (new) — holds `join_via_corewlan` and the moved
  `join_via_networksetup` + `detect_wifi_interface` / `parse_hardware_ports`.
- `src-tauri/src/lib.rs` — keeps the `join_wifi` command, input validation, and
  the startup location request; delegates the actual join to `wifi.rs`.
- `JoinWifiResponse` gains a `method: String` field (`"corewlan"` |
  `"networksetup"`) so the UI/logs show which path ran.

## Bindings

Call CoreWLAN through the `objc2` ecosystem:

- `objc2-core-wlan` — typed `CWWiFiClient` / `CWInterface` / `CWNetwork`.
- `objc2-core-location` — `CLLocationManager` for the permission request.
- `objc2-foundation` — `NSString` / `NSError`.

If `objc2-core-wlan` is missing or incomplete, fall back to raw `objc2`
`msg_send!` against the CoreWLAN classes. **The first implementation step is a
small spike** to confirm the crate exists and that `associate` links before
building the full path.

## Testing

- Existing unit tests (`sanitize_*`, `normalize_security`, `parse_hardware_ports`,
  and the `qr.ts` Vitest suite) stay green — none touch the join.
- The CoreWLAN and `networksetup` calls cannot be meaningfully unit-tested (they
  need real hardware and a live access point), so they are verified manually:
  join a known network and confirm **no admin prompt** appears once location is
  granted, and confirm the response `method` is `"corewlan"`.
- Verify fallback by denying location once and confirming the join still
  succeeds via `networksetup`.

## Risks

1. CoreWLAN `associate` reliability varies across macOS versions — the
   `networksetup` fallback is the safety net.
2. The Location prompt may surprise users who don't expect a Wi-Fi tool to ask
   for location — mitigated by the explanation string and startup timing.
3. `objc2-core-wlan` API surface is the main unknown — resolved by the spike
   before full implementation.
