# WiLens

WiLens is a macOS desktop app that scans a Wi-Fi QR code and joins the network.

## What It Does

1. Opens the camera in a desktop window.
2. Scans a standard Wi-Fi QR code.
3. Shows the network before joining.
4. Uses macOS to switch to that Wi-Fi network.

Example QR payload:

```txt
WIFI:T:WPA;S:MyWifi;P:mypassword;;
```

## Current Status

The MVP is implemented and builds successfully.

Production app bundle:

- `src-tauri/target/release/bundle/macos/WiLens.app`

## Run

Open the built app:

```bash
open src-tauri/target/release/bundle/macos/WiLens.app
```

Run the development build:

```bash
bun install
cargo tauri dev
```

Build production again:

```bash
bun run build
cargo tauri build
```

## Notes

- WiLens currently targets macOS.
- macOS may show an administrator or keychain approval prompt when joining a network because the app uses `networksetup`.
- The system prompt is controlled by macOS, not by the app UI.

## Project Docs

- [Product scope and architecture](docs/product-scope.md)
- [Implementation TODO](docs/todo.md)
