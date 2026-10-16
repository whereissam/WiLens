# WiLens

WiLens is a desktop app for macOS and Windows (beta) that scans a Wi-Fi QR
code — with your camera or from a picture — and joins the network.

[![WiLens intro video](docs/media/wilens-intro.gif)](docs/media/wilens-intro.mp4)

## What It Does

1. Scans a standard Wi-Fi QR code with your camera, or reads one from a
   picture, the clipboard, or drag and drop.
2. Shows the network before joining.
3. Joins the network with no administrator prompt. WPA2, WPA3, WEP, and open
   networks are supported.
   - **macOS:** joins via CoreWLAN, and only joins a network whose security
     matches the QR code.
   - **Windows:** saves a Wi-Fi profile for your account and connects with it,
     never overwriting a profile you already have.
4. Shares a network: type its details to make a Wi-Fi QR code guests can scan,
   and save it as an image. Nothing is stored or sent anywhere.
5. Checks a connection: tests the network link, router, DNS, and internet one
   step at a time, spots sign-in (captive) pages, and explains the result in
   plain language.

Example QR payload:

```txt
WIFI:T:WPA;S:MyWifi;P:mypassword;;
```

## Download

Get the latest build from the
[Releases page](https://github.com/whereissam/WiLens/releases/latest).

### macOS (Apple Silicon)

Download `WiLens.dmg`. The app is ad-hoc signed but not notarized, so macOS
asks you to confirm the first launch:

1. Open `WiLens.dmg` and drag **WiLens** into **Applications**.
2. Launch WiLens. When macOS says it can't verify the developer, click **Done**.
3. Open **System Settings → Privacy & Security**, scroll down, click
   **Open Anyway** next to WiLens, and confirm.

If macOS instead says *"WiLens is damaged and can't be opened"* (builds up to
0.1.2), remove the download quarantine flag and open it again:

```sh
xattr -dr com.apple.quarantine /Applications/WiLens.app
```

### Windows (x64, beta)

Download `WiLens-setup.exe`. It is not signed yet, so SmartScreen asks first:

1. Run `WiLens-setup.exe`. When SmartScreen appears, choose
   **More info → Run anyway**.
2. Allow the camera: **Settings → Privacy & security → Camera → Let desktop
   apps access your camera**.
3. Optional: turn on **Settings → Privacy & security → Location → Let desktop
   apps access your location**. On Windows 11 24H2 and later, WiLens needs it
   to confirm that a join worked.

WPA3-only networks need Windows 10 version 2004 or later.

### Checksums

Each release lists a `.sha256` checksum next to each download. To check the
file you downloaded:

```sh
shasum -a 256 -c WiLens.dmg.sha256
```

```powershell
(Get-FileHash WiLens-setup.exe).Hash; Get-Content WiLens-setup.exe.sha256
```

On Windows, the two hashes should match (ignoring upper/lower case).

## Screenshot

![WiLens app screenshot](web/public/screenshot.png)

## Build and Run

Install the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for
your OS (on Windows: Rust, Bun, and the Visual Studio C++ Build Tools), then:

```bash
bun install
bun run tauri dev     # development build with hot reload
bun run tauri build   # production build
```

### macOS

Local builds produce:

- `src-tauri/target/release/bundle/macos/WiLens.app`
- `src-tauri/target/release/bundle/dmg/WiLens_<version>_aarch64.dmg`

```bash
open src-tauri/target/release/bundle/macos/WiLens.app
```

Published releases get `WiLens.dmg` (with a checksum) from the `macOS build`
workflow, which signs and notarizes it once the `APPLE_*` secrets listed in
`.github/workflows/build-macos.yml` are set. To build the same installer
locally (ad-hoc signed) in one step:

```bash
bun run release:dmg
```

### Windows

Build the installer on Windows:

```powershell
bun install
bun run tauri build --bundles nsis
```

The installer lands in
`src-tauri\target\release\bundle\nsis\WiLens_<version>_x64-setup.exe`, and the
app itself in `src-tauri\target\release\wilens.exe` (runs without installing).
Published releases get `WiLens-setup.exe` (with a checksum) from the
`Windows build` workflow.

You can also cross-compile the Windows installer from a Mac. One-time setup:

```bash
brew install nsis llvm lld
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
```

Then:

```bash
bun run release:win
```

The first run downloads the Windows SDK/CRT libraries. This is Tauri's
experimental cross-compile path; release builds use a real Windows runner.

## Permissions

- **Camera** — to scan the QR code. Not needed if you scan from a picture.
- **Location** — used only to find and confirm your network; WiLens never
  collects, stores, or shares your location.
  - **macOS** requires Location access before *any* app can scan for Wi-Fi
    networks, so it asks the first time you open WiLens. Granting it lets
    WiLens join quietly with no administrator prompt.
  - **Windows 11 24H2 and later** hide the current network name from apps
    without Location access. WiLens still joins, but can't confirm the join or
    read the network's security from a scan.

All permission prompts come from the operating system, not the app.

## Notes

- macOS builds are for Apple Silicon. The Windows (x64) build is in beta.
- On macOS without Location access, WiLens cannot check a secured network, so
  it only joins open networks (through the `networksetup` tool, which may show
  a macOS administrator prompt). For secured networks it asks you to allow
  Location, or to copy the password and join from the Wi-Fi menu.
- On Windows, if a network with the same name is already saved, WiLens uses
  that saved profile and leaves it unchanged. If the saved password is out of
  date, forget the network in Windows Wi-Fi settings and scan again.

## Security & Privacy

WiLens runs entirely on your computer — no servers, no accounts, no tracking.
Its only network traffic is the connection check, and only when you click
**Run check**: it pings your router and fetches a tiny page from
`captive.apple.com` (Windows: `www.msftconnecttest.com`), the same address the
system uses to detect sign-in pages. Camera images, network names, and
passwords never leave your computer. See [SECURITY.md](SECURITY.md) for the full
breakdown and how to verify it yourself.

## Project Docs

- [Product scope and architecture](docs/product-scope.md)
- [Usage guide](docs/usage.md)
- [Roadmap and TODO](docs/todo.md)
- [Changelog](CHANGELOG.md)

## Contributing

Contributions are welcome: see [CONTRIBUTING.md](CONTRIBUTING.md) for setup,
checks, and project principles. Issues labelled
[`good first issue`](https://github.com/whereissam/WiLens/labels/good%20first%20issue)
are a good place to start.

## License

[MIT](LICENSE) © whereissam
