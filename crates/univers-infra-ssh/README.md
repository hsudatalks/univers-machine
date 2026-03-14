# univers-infra-ssh

Workspace SSH infrastructure crate built on `russh`.

Current scope:

- resolve local `~/.ssh/config` aliases and `ProxyJump` chains
- authenticate with configured identity files
- execute remote commands through multi-hop SSH chains
- probe remote HTTP services through the target SSH session
- expose a reusable local port forward handle for remote services
- validate PTY-backed interactive shells and window resize requests
- execute the same remote directory listing and file preview workflow used by the app

Smoke examples:

```bash
cargo run --manifest-path crates/univers-infra-ssh/Cargo.toml \
  --example smoke -- exec automation-dev hostname

cargo run --manifest-path crates/univers-infra-ssh/Cargo.toml \
  --example smoke -- http-probe automation-dev 3432 /

cargo run --manifest-path crates/univers-infra-ssh/Cargo.toml \
  --example smoke -- local-forward-self-test automation-dev 3432 /

cargo run --manifest-path crates/univers-infra-ssh/Cargo.toml \
  --example smoke -- pty-shell-probe automation-dev "hostname"

cargo run --manifest-path crates/univers-infra-ssh/Cargo.toml \
  --example smoke -- list-dir automation-dev ~/repos/hvac-workbench

cargo run --manifest-path crates/univers-infra-ssh/Cargo.toml \
  --example smoke -- preview-file automation-dev ~/repos/hvac-workbench/package.json
```
