# Changelog

All notable changes to WiLens. Releases are published on the
[Releases page](https://github.com/whereissam/WiLens/releases).

## 0.2.0 — Connect, Share, Check

WiLens grows from a QR scanner into a small Wi-Fi companion: join a network,
share one, and find out why the internet isn't working.

### New

- **Share a network.** Type a network's name, security type, and password to
  get a Wi-Fi QR code that guests can scan with a phone camera. Save it as a
  PNG or copy it to the clipboard. The password is never printed on the
  image, and nothing is stored or sent anywhere.
- **Check connection.** One click tests the network link, router, DNS, and
  internet in order and explains the result in plain language. It spots
  hotel and café sign-in pages and offers to open them, and **Copy report**
  gives a summary with no passwords or network names. The check runs only
  when you click it and contacts only your router and the same address your
  OS uses to detect sign-in pages.
- **Scan without a camera.** Scan a QR code from an image file, drop it on
  the window, or paste it (or its `WIFI:` text) with Cmd/Ctrl+V.
- **Windows (beta).** A Windows installer (`WiLens-setup.exe`) that joins
  through a per-user Wi-Fi profile with no administrator prompt. It joins
  WPA3-only networks on Windows 10 version 2004 and later: WiLens reads the
  network's security from a Wi-Fi scan and saves a WPA3 profile when needed.
  If the scan can't tell (for example without Location access), it tries
  WPA3 first for `T:SAE` codes and WPA2 first otherwise.
- WPA3 QR codes (`T:SAE`, `T:WPA3`) show "WPA3" in the network details.
- After a failed join, a **Check connection** button runs the check
  straight away.

### Security

From a full security audit of the app:

- macOS only joins a network whose security matches the QR code, so a nearby
  same-name network with weaker security (an "evil twin") is refused.
- Without Location access, macOS only joins open networks through the
  `networksetup` fallback, so passwords never appear on a command line and a
  saved network's password can't be replaced.
- Windows never overwrites or deletes a Wi-Fi profile you saved yourself, and
  refuses network names that netsh would treat as wildcards.
- Hidden or direction-changing characters in network names are shown, so a
  QR code can't disguise which network you're joining.
- Stricter content security policy, SHA-pinned CI actions, read-only CI
  tokens by default, and SHA-256 checksums published with each download.

### Fixes

- The app bundle is ad-hoc signed, so macOS no longer reports it as damaged.
- Joins over the fallback now say when they couldn't be confirmed, instead
  of always reporting success.
- Network names with leading or trailing spaces join correctly on Windows.
- On Windows, camera errors name the setting to turn on, and a missing or
  busy camera gets its own message. A join that can't be confirmed points to
  the Location setting Windows 11 24H2 and later need.

### Known limitations

- The app is not yet notarized, so macOS asks you to confirm the first
  launch (System Settings → Privacy & Security → Open Anyway).
- Joining a secured network on macOS needs Location access, so WiLens can
  check the network's security.
- Enterprise (802.1X / EAP) networks are not supported.
- The Windows installer is not signed yet, so SmartScreen asks first
  (More info → Run anyway).

## 0.1.2 — Faster, clearer Wi-Fi joins

- Fails fast on a wrong password, shows live progress while joining, and
  avoids false "Joined" reports from the `networksetup` fallback.

## 0.1.1 — Join without an admin password

- Joins through CoreWLAN, so normal joins no longer ask for an administrator
  password.

## 0.1.0

- First release: scan a Wi-Fi QR code with the camera and join the network.
