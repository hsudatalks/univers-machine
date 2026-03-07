# univers-ark-developer

`univers-ark-developer` is a local desktop shell for switching between developer targets, keeping a remote terminal on the left and browser surfaces on the right.

This project lives inside `univers-machine/apps` because it is meant to orchestrate local machine workflows:

- target selection
- SSH attach commands
- browser tunnel commands
- a dedicated right-hand browser pane with development and preview switching
- future Tauri hooks for PTY management and tunnel supervision

## Stack

- Tauri 2
- React 19
- TypeScript
- Vite

## Current shape

The first version is a runnable shell with:

- a target rail
- a remote-console pane
- a browser pane rendered inside an `iframe`
- per-target development and preview surfaces
- a JSON-backed target config
- Tauri backend commands that load the config and reserve command hooks for tunnel/session actions

The current UI is intentionally opinionated so the app already feels like a focused control desk instead of a generic admin dashboard.

## Default target

The seed profile points at `automation-dev`:

- terminal: `ssh automation-dev`
- development tunnel template: `ssh -NT -L {localPort}:127.0.0.1:3432 automation-dev`
- development HMR tunnel template: `ssh -NT -L {localPort}:127.0.0.1:3433 automation-dev`
- preview tunnel template: `ssh -NT -L {localPort}:127.0.0.1:4173 automation-dev`
- browser surfaces: runtime-mapped to free local ports in `43000-43999`
- development surfaces with `viteHmrTunnelCommand` are exposed through a local proxy that rewrites Vite's `@vite/client` HMR port to the runtime-mapped browser port

Edit [`developer-targets.json`](./developer-targets.json) to add more targets.

## Development

Install dependencies:

```bash
pnpm install
```

Run the web shell only:

```bash
pnpm dev
```

Run the desktop app:

```bash
pnpm tauri:dev
```

Build the frontend:

```bash
pnpm build
```

Build the desktop app:

```bash
pnpm tauri:build
```

## Structure

```text
src/              React UI shell
src/lib/          Frontend helpers for Tauri invoke calls
src-tauri/        Tauri backend and app config
developer-targets.json
```

## Next steps

Natural follow-ups for this project:

1. Add PTY-backed terminal streaming with a sidecar process
2. Persist per-target surface preference, sessions, and health
3. Add richer tunnel diagnostics and retry strategy when a preferred port becomes unavailable
4. Add per-surface actions for opening dev tools, logs, and health checks
