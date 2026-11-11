Krake läuft als Server mit Web-Oberfläche (Docker oder einzelnes Binary), als Desktop-App oder als Kommandozeilen-Werkzeug. Such dir einen Weg aus; sie lassen sich kombinieren.

## Docker

```bash
docker run -d --name krake \
  -p 1516:1516 \
  -v ./config:/config -v ./downloads:/downloads \
  -e KRAKE_BIND=0.0.0.0:1516 \
  ghcr.io/amigo-labs/krake:latest
```

Öffne `http://localhost:1516`. Der Einrichtungsassistent legt das Admin-Konto an.

Click'n'Load (Port 9666, für Browser-Erweiterungen) funktioniert mit dem Container nicht: Der Dienst nimmt nur Verbindungen von dem Rechner an, auf dem er läuft, und das ist im Container der Container selbst. Es funktioniert, wenn `krake-server` oder die Desktop-App auf demselben Rechner läuft wie der Browser.

Dasselbe als Compose-Datei:

```yaml
services:
  krake:
    image: ghcr.io/amigo-labs/krake:latest
    ports:
      - "1516:1516"   # Web-Oberfläche
    volumes:
      - ./config:/config
      - ./downloads:/downloads
      - ./plugins:/etc/krake/plugins   # optional: eigene Plugins
    environment:
      - KRAKE_BIND=0.0.0.0:1516
```

| Variable | Zweck |
|----------|-------|
| `KRAKE_BIND` | Adresse, auf der der Server lauscht. Standard ist nur Loopback, ein Container braucht deshalb `0.0.0.0:1516`. |
| `KRAKE_SETUP_USER`, `KRAKE_SETUP_PASSWORD` | Legen das Admin-Konto beim Start an und überspringen den Assistenten. |
| `KRAKE_SETUP_PIN` | Der Assistent fragt zuerst nach dieser PIN. Setze sie, wenn der Server aus dem Internet erreichbar ist. |
| `KRAKE_TRUST_PROXY` | `true` hinter einem Reverse-Proxy, damit weitergeleitete Adressen gelten. |
| `KRAKE_AUTO_UPDATE_PLUGINS` | `true` installiert Plugin-Updates automatisch. |

## Server-Binary

Lade `krake-server` für deine Plattform aus den [Releases](https://github.com/amigo-labs/amigo-krake/releases), entpacke es und starte es. Im Archiv liegt auch `krake-plugin-host`; beide Dateien gehören in denselben Ordner. Die Web-Oberfläche ist eingebaut.

```bash
./krake-server
```

Der Server legt seine Datenbank (`krake.db`) in dem Ordner an, aus dem er gestartet wird, und `config.toml` ebenfalls dort, außer `KRAKE_CONFIG_DIR` nennt einen anderen Ordner.

## Kommandozeile

Unter Linux und macOS (x86_64 und aarch64):

```bash
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-krake/main/scripts/install.sh | bash
```

Das Skript nimmt `krake` aus dem neuesten Release, prüft die SHA-256-Prüfsumme und installiert nach `/usr/local/bin` (änderbar mit `KRAKE_INSTALL_DIR`). Unter Windows lädst du das CLI-Archiv aus den Releases.

## Desktop-App

Installer für Windows, macOS und Linux hängen an jedem Release. Die App bringt den Server mit und zeigt dieselbe Oberfläche in einem eigenen Fenster.

## Auf dem Handy

Die Web-Oberfläche läuft in jedem Handy-Browser. Um sie als App zu installieren und Links an sie zu teilen, muss der Server über HTTPS mit einem Zertifikat erreichbar sein, dem das Handy vertraut (zum Beispiel hinter einem Reverse-Proxy; setze `KRAKE_TRUST_PROXY=true`), und der Browser muss das Teilen an installierte Web-Apps unterstützen (Chrome unter Android kann das).

1. Öffne `https://dein-server` im Browser des Handys.
2. Wähle „Zum Home-Bildschirm“ bzw. „App installieren“.
3. Teile einen Link aus einer beliebigen App an Krake. Er landet in der Warteschlange.

## Prüfsummen

Jedes Release enthält eine Datei `SHA256SUMS` mit der Prüfsumme jedes Downloads:

```bash
sha256sum --check --ignore-missing SHA256SUMS
```

## Aus dem Quelltext bauen

Du brauchst Rust (die Version aus `rust-toolchain.toml`), Node.js mit pnpm, `protoc` und `clang` mit WebAssembly-Target.

```bash
# Debian / Ubuntu
sudo apt-get install protobuf-compiler clang lld
# macOS: Apples clang hat kein WebAssembly-Target, deshalb den Build auf das von Homebrew zeigen lassen
brew install protobuf llvm
export CC_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/clang"
export AR_wasm32_unknown_unknown="$(brew --prefix llvm)/bin/llvm-ar"

pnpm install && pnpm build:web-ui
cargo build --bin krake-plugin-host    # Plugins laufen in diesem Prozess; der Server sucht ihn neben sich
cargo run --bin krake-server
```

Ein Release-Build (`--release`, `cargo install`) braucht zwei Signaturschlüssel, `KRAKE_UPDATE_PUBKEY_HEX` und `KRAKE_REGISTRY_PUBKEY_HEX`. Sie legen fest, wessen Updates und Plugin-Registry das Binary vertraut. Mit eigenen Schlüsseln schlägt die Prüfung der offiziellen Updates und der Registry fehl. Debug-Builds laufen ohne Schlüssel und überspringen die Signaturprüfung mit einer Warnung.
