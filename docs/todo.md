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

---

## Phase 7: Post-Review Improvements

Derived from the engineering / security / design review (2026-06-19).
Business items intentionally dropped — WiLens is an open-source utility.

### 7a. Testing (highest risk: zero coverage on the most logic-heavy code)

- [x] Add a test runner (Vitest) to the frontend.
- [x] Test `parseWifiQr` with WPA/WPA2 payloads.
- [x] Test `parseWifiQr` with open (`nopass`) networks.
- [x] Test escaped characters in SSID/password (`\;`, `\:`, `\\`, `\,`).
- [x] Test malformed / partial QR strings (missing `WIFI:`, missing SSID, missing password, bad security type).
- [x] Add Rust unit tests for `sanitize_required`, `sanitize_optional`, `normalize_security`.
- [x] Improve open-network handling: infer missing/empty `T` from password presence instead of defaulting to WPA.

### 7b. Reliability

- [x] Verify `networksetup -setairportnetwork` works on the target macOS version. *(kept as fallback; primary join now uses CoreWLAN)*
- [x] Document the `Copy password` manual fallback as the supported Plan B if the join command fails. *(already in `docs/usage.md`)*
- [x] Replace the per-join admin prompt by joining via CoreWLAN; request Location permission at startup; fall back to networksetup. *(see `docs/superpowers/specs/2026-06-20-corewlan-join-design.md`)*

### 7c. Security hardening

- [x] Set a restrictive CSP in `tauri.conf.json` (replace `"csp": null`).
- [x] Reject SSID/password values beginning with `-` to prevent `networksetup` argument injection.
- [x] Run a dependency audit. *(remaining advisories are dev-only transitive deps — vite/vitest → picomatch — not shipped in the app)*

### 7d. Frontend / accessibility

- [x] Add `aria-live="polite"` to the `#status` element so screen readers announce scan/join results.

### 7e. Landing page

- [x] Replace the dead `githubHref = "#"` links with the real repo URL (or remove them).
- [x] Delete the leftover commented-out `signal-logo` markup.
- [x] Fix the broken favicon reference (`/app-logo.svg` did not exist) and wire in the new brand assets.

### 7f. Tooling / CI

- [x] Add a CI workflow: `tsc --noEmit`, `vitest`, `cargo test`, `cargo clippy`.
