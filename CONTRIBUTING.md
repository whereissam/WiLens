# Contributing to WiLens

Thanks for helping! WiLens is a small, privacy-first desktop app for Wi-Fi:
**Connect** (scan a Wi-Fi QR code and join), **Share** (make a QR code for
guests), and **Check** (find out why the internet isn't working). Bug reports,
tests, translations, docs, and fixes are all welcome.

New here? Look for issues labelled
[`good first issue`](https://github.com/whereissam/WiLens/labels/good%20first%20issue).
They are scoped so you can finish them without knowing the whole codebase.

## Set up

You need [Bun](https://bun.sh), [Rust](https://rustup.rs) (stable), and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```bash
bun install
bun run tauri dev     # run the app with hot reload
```

The camera, Wi-Fi joins, and the connection check only fully work inside the
app. For UI-only work, `bun run dev` serves the frontend in a browser, where
everything except the Tauri commands works.

## Before you open a pull request

Run the same checks CI runs:

```bash
bunx tsc --noEmit
bun run test
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI also runs clippy and tests on Windows, so you don't need a Windows machine
to change `netsh.rs`, but please say in the PR what you could and couldn't test.

## Where things live

| Path | What it does |
| --- | --- |
| `src/main.ts` | The UI: Join / Share / Check tabs and their state |
| `src/qr.ts` | Parse and build `WIFI:` QR payloads (well tested; a good place to start) |
| `src/share.ts` | Draw a QR code onto a canvas and export it |
| `src-tauri/src/lib.rs` | Tauri commands and input validation |
| `src-tauri/src/wifi.rs` | macOS joins (CoreWLAN, `networksetup` fallback) |
| `src-tauri/src/netsh.rs` | Windows joins (`netsh wlan` profiles) |
| `src-tauri/src/diagnose.rs` | The connection check |
| `docs/todo.md` | Roadmap and what's planned next |

## Principles

These keep WiLens trustworthy. Please keep them in mind when proposing changes:

- **Privacy first.** No telemetry, analytics, accounts, or cloud services. The
  only network traffic is the connection check, and only when the user asks.
- **Don't store secrets.** Passwords are used for the current operation and
  then dropped. No saved vaults or network history without a design discussion.
- **Narrow commands.** The webview talks to Rust through small, typed Tauri
  commands. Don't add commands that run arbitrary programs, open arbitrary
  URLs, or read and write arbitrary paths.
- **Untrusted input.** Anything from a QR code, an image, or the clipboard is
  untrusted. Validate it in Rust as well as in the frontend.
- **Plain language.** Error messages and results should tell people what
  happened and what to do next, without jargon.
- **Small and focused.** Wi-Fi connect, share, and check. Features outside
  that (file transfer, spectrum analysis) belong in other tools.

## Proposing a feature

Open a feature request first, so we can agree on the approach before you write
code. Check [`docs/todo.md`](docs/todo.md) to see whether it's already planned
or deliberately postponed.

## Reporting bugs

Use the bug report form. For connection problems, the **Copy report** button in
the Check tab produces a summary you can paste. It contains no passwords or
network names. Never paste a real Wi-Fi password or a real network's QR code
into an issue.

## Security issues

Please report vulnerabilities privately through a
[security advisory](https://github.com/whereissam/WiLens/security/advisories/new).
See [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
