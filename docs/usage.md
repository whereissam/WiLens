# WiLens Usage

## Open the App

If you already built the app locally, open it with:

```bash
open src-tauri/target/release/bundle/macos/WiLens.app
```

If macOS blocks first launch:

1. Open Finder.
2. Go to `src-tauri/target/release/bundle/macos/`.
3. Right-click `WiLens.app`.
4. Choose `Open`.

## What WiLens Does

WiLens scans a Wi-Fi QR code and helps your Mac join that network.

Supported QR format:

```txt
WIFI:T:WPA;S:MyWifi;P:mypassword;;
```

## How To Use It

1. Open WiLens.
2. Click `Start camera`.
3. Allow camera access if macOS asks.
4. Point the camera at a Wi-Fi QR code.
5. Wait for WiLens to detect the network.
6. Review the scanned network details.
7. Click `Join network`.
8. Approve any macOS prompt if shown.

## Expected macOS Prompts

WiLens may trigger system prompts for:

- camera permission
- administrator approval
- keychain or networking permission

This happens because the app uses macOS system networking tools to join Wi-Fi.

## Copy Password Fallback

If joining does not work as expected, use `Copy password` and connect manually from macOS Wi-Fi settings.

## Troubleshooting

### Camera does not start

- Check macOS camera permissions for the app.
- Restart the app and try again.

### QR code is not detected

- Move closer to the QR code.
- Improve lighting.
- Make sure the QR code contains a Wi-Fi payload.

### Network join fails

- Verify the QR code uses the correct SSID and password.
- Check whether macOS showed an approval dialog behind the app.
- Try manual connection using the copied password.

### App says success but macOS showed a prompt

That can happen. The Wi-Fi join result and the macOS authorization dialog are controlled by the system, not fully by the app UI.
