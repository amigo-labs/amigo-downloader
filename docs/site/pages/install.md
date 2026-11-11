Krake runs as a server with a Web UI (Docker or a single binary), as a desktop app, or as a command-line tool. Pick one; they can be combined.

## Docker

```bash
docker run -d --name krake \
  -p 1516:1516 \
  -v ./config:/config -v ./downloads:/downloads \
  -e KRAKE_BIND=0.0.0.0:1516 \
  ghcr.io/amigo-labs/krake:latest
```

Open `http://localhost:1516`. The setup wizard creates the admin account.

Click'n'Load (port 9666, for browser extensions) does not work with the container: the listener only accepts connections from the machine it runs on, which inside a container is the container itself. It works when `krake-server` or the desktop app runs on the same machine as the browser.

The same as a compose file:

```yaml
services:
  krake:
    image: ghcr.io/amigo-labs/krake:latest
    ports:
      - "1516:1516"   # Web UI
    volumes:
      - ./config:/config
      - ./downloads:/downloads
      - ./plugins:/etc/krake/plugins   # optional: your own plugins
    environment:
      - KRAKE_BIND=0.0.0.0:1516
```

| Variable | Purpose |
|----------|---------|
| `KRAKE_BIND` | Address the server listens on. The default is loopback only, so a container needs `0.0.0.0:1516`. |
| `KRAKE_SETUP_USER`, `KRAKE_SETUP_PASSWORD` | Create the admin account at start and skip the wizard. |
| `KRAKE_SETUP_PIN` | The wizard asks for this PIN first. Set it when the server is reachable from the internet. |
| `KRAKE_TRUST_PROXY` | `true` behind a reverse proxy, so forwarded addresses are trusted. |
| `KRAKE_AUTO_UPDATE_PLUGINS` | `true` installs plugin updates automatically. |

## Server binary

Download `krake-server` for your platform from the [releases](https://github.com/amigo-labs/amigo-krake/releases), unpack it and run it. The archive also holds `krake-plugin-host`; keep both files in the same folder. The Web UI is built in.

```bash
./krake-server
```

The server keeps its database (`krake.db`) in the folder it is started from, and `config.toml` there too unless `KRAKE_CONFIG_DIR` names another folder.

## Command line

On Linux and macOS (x86_64 and aarch64):

```bash
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-krake/main/scripts/install.sh | bash
```

The script takes `krake` from the latest release, checks its SHA-256 checksum and installs it to `/usr/local/bin` (change with `KRAKE_INSTALL_DIR`). On Windows, download the CLI archive from the releases.

## Desktop app

Installers for Windows, macOS and Linux are attached to each release. The app bundles the server and opens the same interface in its own window.

## On your phone

The Web UI works in any phone browser. To install it as an app and share links to it, the server must be reachable over HTTPS with a certificate the phone trusts (for example behind a reverse proxy; set `KRAKE_TRUST_PROXY=true`), and the browser must support sharing to installed web apps (Chrome on Android does).

1. Open `https://your-server` in the phone's browser.
2. Choose "Add to Home Screen" / "Install app".
3. Share a link from any app to Krake. It lands in the queue.

## Checksums

Every release carries a `SHA256SUMS` file with the checksum of each download:

```bash
sha256sum --check --ignore-missing SHA256SUMS
```

## Build from source

You need Rust (the version in `rust-toolchain.toml`), Node.js with pnpm, `protoc`, and `clang` with a WebAssembly target.

```bash
# Debian / Ubuntu
sudo apt-get install protobuf-compiler clang lld
# macOS: Apple's clang has no WebAssembly target, so point the build at Homebrew's
brew install protobuf llvm
export CC_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/clang"
export AR_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/llvm-ar"

pnpm install && pnpm build:web-ui
cargo build --bin krake-plugin-host    # plugins run in this process; the server looks for it next to itself
cargo run --bin krake-server
```

A release build (`--release`, `cargo install`) needs two signing keys, `KRAKE_UPDATE_PUBKEY_HEX` and `KRAKE_REGISTRY_PUBKEY_HEX`. They decide whose updates and plugin registry the binary trusts. With keys of your own, the official updates and registry fail verification. Debug builds run without keys and skip signature checks with a warning.
