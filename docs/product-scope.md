# WiLens Product Scope

## Product Goal

WiLens is a small macOS desktop utility for joining Wi-Fi networks from a QR code.

The MVP keeps the architecture intentionally simple:

- `Tauri` provides the desktop shell.
- The frontend handles camera access and QR scanning with web APIs.
- Rust handles the privileged macOS step: joining the scanned Wi-Fi network.

## Scope

### Version 1

The first release focuses on the shortest path to a working app:

- small Tauri desktop window,
- camera permission prompt in the frontend,
- QR scanning in the webview using a browser-friendly library,
- Wi-Fi QR parsing,
- Rust command that shells out to:

```bash
networksetup -setairportnetwork <interface> <ssid> <password>
```

- clear success and failure states.

### Version 2

After the MVP works reliably:

- auto-detect the Wi-Fi interface,
- show the scanned SSID before joining,
- keep recent scans locally,
- provide a fallback action to copy credentials,
- improve malformed QR handling.

### Version 3

Optional native direction:

- native camera capture in Rust,
- `nokhwa` for camera frames,
- Apple Vision barcode detection via Rust bindings.

This is out of scope for the first build.

## Architecture

### Frontend

Responsibilities:

- request camera permission,
- show live preview,
- scan QR codes,
- parse Wi-Fi QR strings,
- present confirmation UI,
- invoke a Tauri command with structured network data.

Implemented files:

- `src/main.ts`
- `src/qr.ts`
- `src/styles.css`

### Rust / Tauri

Responsibilities:

- expose a Tauri command such as `join_wifi`,
- validate and sanitize frontend input,
- detect the Wi-Fi interface,
- invoke `networksetup`,
- return structured success and error messages.

Implemented files:

- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`

## Risks

- camera permission behavior in the Tauri webview,
- QR payloads that do not follow the expected Wi-Fi format,
- macOS-specific behavior of `networksetup`,
- Wi-Fi interface detection differences across machines,
- harder distribution constraints if App Store delivery is ever required.

## macOS Permission Behavior

WiLens changes the current Wi-Fi network by invoking `networksetup` from the Tauri backend. On macOS, that can trigger a system authorization dialog asking for administrator approval or keychain access.

This prompt is owned by macOS, not by WiLens. It may still appear even when the join succeeds.

## Build Notes

Verified locally:

- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo tauri build`
