## What does this change?

<!-- A sentence or two, and the issue it closes (e.g. "Closes #12"). -->

## How was it tested?

- [ ] `bunx tsc --noEmit` and `bun run test`
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (in `src-tauri/`)
- [ ] Tried in the app on: <!-- macOS / Windows, or "not tested in the app" -->

## Checklist

- [ ] No passwords, network names, or other secrets are logged, stored, or sent anywhere.
- [ ] New Tauri commands (if any) take typed input and can't run arbitrary programs, open arbitrary URLs, or touch arbitrary files.
- [ ] Docs updated if behavior changed (`README.md`, `docs/usage.md`, `docs/todo.md`).
