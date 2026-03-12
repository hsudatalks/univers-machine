# Ark Developer Debian Packaging

`Ark Developer` now targets Debian package output only on Linux.

## What the package contains

- The Tauri desktop application.
- Bundled resources declared in [src-tauri/tauri.conf.json](./src-tauri/tauri.conf.json):
  - `developer-targets.json`
  - `resources/daemons/`

The daemon archives must be generated before packaging so the `.deb` contains the current bundled installer assets.

## Build command

From the workspace root:

```bash
scripts/build-ark-deb.sh
```

This does two things:

1. Builds the four bundled daemon archives:
   - `univers-machine-daemon-x86_64-unknown-linux-gnu.tar.gz`
   - `univers-machine-daemon-aarch64-unknown-linux-gnu.tar.gz`
   - `univers-container-daemon-x86_64-unknown-linux-gnu.tar.gz`
   - `univers-container-daemon-aarch64-unknown-linux-gnu.tar.gz`
2. Runs `pnpm tauri build`, which now produces `.deb` bundles only.

## Output path

The generated Debian package is written under:

```text
target/release/bundle/deb/
```

## Architecture behavior

The `.deb` output architecture follows the host you build on:

- Build on Ubuntu `x86_64` to get an `amd64` package.
- Build on Ubuntu `aarch64` to get an `arm64` package.

The bundled daemon assets always include both remote Ubuntu daemon architectures, independent of the local desktop package architecture.

## Notes

- `.tar.gz` daemon assets under `src-tauri/resources/daemons/` are generated files and are git-ignored.
- If you later want to add custom Debian maintainer scripts or desktop file overrides, put them under `bundle.linux.deb` in `tauri.conf.json`.
