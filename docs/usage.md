# WiLens Usage

## Open the App

Install WiLens from the
[Releases page](https://github.com/whereissam/WiLens/releases/latest), or open
a local build.

**macOS:**

```bash
open src-tauri/target/release/bundle/macos/WiLens.app
```

If macOS blocks first launch:

1. Open Finder.
2. Go to `src-tauri/target/release/bundle/macos/`.
3. Right-click `WiLens.app`.
4. Choose `Open`.

**Windows:** run `WiLens-setup.exe` to install it, or start
`src-tauri\target\release\wilens.exe` directly without installing. The
installer isn't signed yet, so SmartScreen asks first: choose
**More info → Run anyway**.

## What WiLens Does

WiLens scans a Wi-Fi QR code and helps your computer join that network. It
supports WPA2, WPA3, WEP, and open networks.

Supported QR format:

```txt
WIFI:T:WPA;S:MyWifi;P:mypassword;;
```

`T:SAE` and `T:WPA3` codes (WPA3, as made by Android phones) work too.

## How To Use It

1. Open WiLens.
2. Click `Start camera`.
3. Allow camera access if your system asks.
4. Point the camera at a Wi-Fi QR code.
5. Wait for WiLens to detect the network.
6. Review the scanned network details.
7. Click `Join network`.
8. On macOS, approve any system prompt if shown.

No camera handy? Click `Scan an image…`, drop a picture of the QR code onto the
window, or paste one with Cmd+V (Ctrl+V on Windows). Pasting the `WIFI:` text
itself works too. If joining fails, a `Check connection` button runs the
connection check straight away.

## Share a Network

Switch to **Share a network**, type the network name, security type, and
password, and WiLens draws a standard Wi-Fi QR code as you type. **Save image…**
asks where to save a PNG (the network name is printed under the code; the
password is not); **Copy image** puts it on the clipboard instead. Anyone who
sees the code can join the network, so share it
only with people you trust. WiLens does not keep the details after you close it.

## Check a Connection

Switch to **Check connection** and click **Run check**. WiLens tests four things
in order and explains where the connection stops working:

1. **Network connection:** is this computer on a network at all?
2. **Router:** does the router answer? (Some routers ignore this check, so a
   silent router only counts against the connection if later steps fail too.)
3. **Website names (DNS):** can names like `captive.apple.com` be looked up?
4. **Internet:** does the internet answer, or does a sign-in page answer
   instead (common in hotels, airports, and cafés)?

If a sign-in page is found, **Open sign-in page** opens it in your browser.
**Copy report** copies the result as text to paste into a support request. It
contains no passwords or network names. The check only contacts your router and
`captive.apple.com` (Windows: `www.msftconnecttest.com`), and only when you
click the button.

## Expected macOS Prompts

WiLens may trigger system prompts for:

- **Camera** — to scan the QR code.
- **Location** — macOS requires Location access before *any* app can scan for
  Wi-Fi networks. WiLens only uses it to find and join your network; it never
  collects, stores, or shares your location. Granting it lets WiLens join
  quietly via CoreWLAN, with no administrator prompt.
- **Administrator approval** — only as a fallback for open networks. If the
  quiet CoreWLAN join can't run (e.g. Location was denied), WiLens joins open
  networks with the `networksetup` tool, which may ask for an administrator
  name and password. Secured networks need Location access so WiLens can check
  the network's security; otherwise use Copy password below.

## Windows Settings

Windows doesn't always ask; desktop apps follow these switches in
**Settings → Privacy & security**:

- **Camera → Let desktop apps access your camera** — needed to scan with the
  camera. Scanning from a picture works without it.
- **Location → Let desktop apps access your location** — optional. On
  Windows 11 24H2 and later, WiLens needs it to confirm that a join worked and
  to read a network's security from a Wi-Fi scan. Without it WiLens still
  joins, but may say it "could not confirm the connection"; check the Wi-Fi
  icon in the taskbar.

WiLens never needs administrator rights on Windows. It saves a Wi-Fi profile
for your account only and connects with it.

## Copy Password Fallback

If joining does not work as expected, use `Copy password` and connect manually
from your system's Wi-Fi settings.

## Troubleshooting

### Camera does not start

- **macOS:** check the app's camera permission in System Settings → Privacy &
  Security → Camera.
- **Windows:** turn on Settings → Privacy & security → Camera → Let desktop
  apps access your camera, and close other apps using the camera.
- No camera? Scan from a picture instead (see above).
- Restart the app and try again.

### QR code is not detected

- Move closer to the QR code.
- Improve lighting.
- Make sure the QR code contains a Wi-Fi payload.

### Network join fails

- Verify the QR code uses the correct SSID and password.
- **macOS:** check whether macOS showed an approval dialog behind the app.
- **Windows:** if this PC already has a saved network with the same name,
  WiLens uses that saved profile and won't change it. If its password is out of
  date, forget the network in Windows Wi-Fi settings and scan again.
- **Windows:** WPA3-only networks need Windows 10 version 2004 or later and a
  Wi-Fi adapter that supports WPA3.
- **Windows:** network names containing `*`, `?`, or `"` can't be joined.
- Try manual connection using the copied password.

### App says success but macOS showed a prompt

That can happen. The Wi-Fi join result and the macOS authorization dialog are controlled by the system, not fully by the app UI.
