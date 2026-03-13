# WiLens TODO

## Phase 0: Project Setup

- [x] Initialize the Tauri project with a TypeScript frontend.
- [x] Confirm the app builds locally on macOS.
- [x] Add basic project metadata, app name, and bundle identifiers.
- [x] Replace the default template with a minimal custom UI.

## Phase 1: QR Scanning MVP

- [x] Add a browser-based QR scanning library such as `@zxing/browser`.
- [x] Build a small single-screen UI with camera preview.
- [x] Request camera permission and handle denial cleanly.
- [x] Detect and scan QR codes continuously from the live camera stream.
- [x] Stop scanning once a valid Wi-Fi QR payload is found.

## Phase 2: QR Parsing

- [x] Create `src/qr.ts` for Wi-Fi QR parsing.
- [x] Support the standard format: `WIFI:T:<type>;S:<ssid>;P:<password>;;`
- [x] Validate required fields before calling Rust.
- [x] Handle escaped characters and malformed payloads defensively.
- [x] Show the parsed SSID, security type, and password state before joining.

## Phase 3: macOS Wi-Fi Join

- [x] Create a Tauri command in Rust for joining Wi-Fi.
- [x] Detect the Wi-Fi interface in Rust before running `networksetup`.
- [x] Call `networksetup -setairportnetwork`.
- [x] Capture stdout, stderr, and exit status.
- [x] Return structured error messages to the frontend.

## Phase 4: UI Flow

- [x] Show a scan result card with SSID and security type.
- [x] Require explicit user confirmation before joining.
- [x] Show pending, success, and failure states.
- [x] Add a retry path after failed scans or failed join attempts.
- [x] Add a manual copy-password fallback.

## Phase 5: Testing

- [ ] Test with WPA/WPA2 QR codes.
- [ ] Test with open network QR codes.
- [ ] Test malformed and partial QR strings.
- [ ] Test camera permission denied on first launch.
- [ ] Test `networksetup` behavior on the target macOS version.
- [ ] Verify behavior when the machine is already connected to another network.

## Phase 6: Hardening

- [x] Prevent duplicate join attempts while a connection request is in flight.
- [x] Avoid logging raw passwords.
- [x] Sanitize command inputs before invoking shell commands.
- [x] Add frontend validation and backend validation for defense in depth.
- [x] Document macOS limitations and expected permissions.

## Nice-to-Have

- [ ] Auto-detect the Wi-Fi interface.
- [ ] Save recent scans locally.
- [ ] Keep a short connection history.
- [ ] Add a recent-networks screen.
- [ ] Add optional native scanning research with `nokhwa` and Vision.

## Known Behavior

- [x] `networksetup` may trigger a macOS administrator or keychain approval prompt during join.
- [ ] Verify whether the prompt can be reduced or better explained for repeated joins on the same machine.

## Definition of Done for MVP

- [x] App launches as a Tauri desktop window on macOS.
- [x] Camera preview works.
- [x] A standard Wi-Fi QR code can be scanned.
- [x] Parsed network details are shown before joining.
- [x] The app attempts Wi-Fi join through Rust successfully.
- [x] Errors are understandable enough to debug field issues.
