# Univers Ark Developer iOS Migration

This app now has an initialized Tauri iOS target under `src-tauri/gen/apple`.

## Current status

- Tauri mobile entry wiring is in place.
- Vite dev server can bind to `TAURI_DEV_HOST` for device testing.
- iOS project scaffolding has been generated with `tauri ios init`.
- Desktop-only menu wiring is isolated from mobile builds.
- Config bootstrap can use the app sandbox directory on mobile instead of `~/.univers`.

## Commands

```bash
pnpm --dir apps/univers-ark-developer tauri:ios:init
pnpm --dir apps/univers-ark-developer tauri:ios:dev
pnpm --dir apps/univers-ark-developer tauri:ios:build
```

## Remaining blockers

- No Apple development team is configured for code signing yet.
- Frontend currently targets `@xterm/xterm` 6.x.
- Clipboard commands return a mobile placeholder error.
- Local-machine bootstrap, local shell commands, `gh`, `tailscale`, and tmux-oriented workflows are still desktop-first and need a mobile-safe product decision.
- The current React layout is still desktop-oriented and needs a dedicated mobile navigation model.
