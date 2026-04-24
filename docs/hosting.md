# Hosting amigo-downloader

amigo ships three first-class deployment shapes. Pick the one that matches
where the server actually runs.

## Which variant do I need?

| Variant | Runs standalone? | Needs a server too? | Typical use |
|---|---|---|---|
| **Tauri Desktop** (`amigo-desktop`) | yes — spawns an internal loopback server | no | Personal laptop/desktop, single user |
| **`amigo-server` binary** (Linux/Mac/Windows) | yes — ships the Web UI embedded | no | Self-hosted on a NAS / homelab without containers |
| **Docker image** (`amigo-downloader:latest`) | yes (as a container) | no | Always-on homeserver, multiple clients |
| **`amigo-dl <URL>`** (direct download) | yes — one shot, no daemon | no | Quick single-file download, `yt-dlp`-style |
| **Web UI in browser** | no — it's only a frontend | yes — served by `amigo-server` or Docker | Regular GUI use |
| **`amigo-dl add / list / pause / …`** (queue mode) | no | yes — local `amigo-server` or a remote reachable via `amigo-dl login` | Scripted queue management |
| **`amigo-dl login <url>` + `remote …`** | no | yes — a remote `amigo-server` (usually Docker) | Laptop → home server |

In short:

- **Standalone** (no extra service required): Tauri, the `amigo-server` binary, the Docker container, `amigo-dl <URL>`.
- **Needs a running server** (Docker or local, doesn't matter which): browser UI, queue CLI (`add`, `list`, …), remote CLI (`login`, `--remote`).

## Local (default)

Running `amigo-server` (or the Tauri desktop app) with no extra config
binds to `127.0.0.1:1516`. Authentication is off because nothing on the
network can reach the process — the wizard, login page, and pairing flow
are all skipped. Just open <http://localhost:1516> and use the app.

## Docker / LAN (first-run wizard)

```sh
cd docker && docker compose up -d
```

`docker-compose.yml` sets `AMIGO_BIND=0.0.0.0:1516`, so the container
listens on every interface. On first start the server enters **setup
mode**: any HTTP request to `/api/v1/*` returns 503 `{"error":
"setup_required"}`, and the Web UI automatically shows the setup wizard.

1. Open `http://<docker-host>:1516` in a browser.
2. Create the admin account (username + password). That's it — a session
   cookie is issued, the setup flag is persisted to
   `/config/config.toml`, and the app loads.

First-visitor-wins (trust-on-first-use) is the default, matching Home
Assistant / Jellyfin / Sonarr. For public-internet deployments where that
isn't acceptable, see below.

### Provisioning options (env vars)

| Variable | Effect |
| --- | --- |
| `AMIGO_BIND` | Listen address. Default `127.0.0.1:1516` locally, `0.0.0.0:1516` in Docker. |
| `AMIGO_SETUP_PIN` | When set, the wizard requires `X-Setup-Pin: <value>` on `/api/v1/setup/*`. Recommended for public VPS / Tailscale Funnel. |
| `AMIGO_SETUP_USER` + `AMIGO_SETUP_PASSWORD` | Headless provisioning — the admin account is created at startup and the wizard never appears. Good for IaC / docker-compose. |
| `AMIGO_TRUST_PROXY` | `true` = honour `X-Forwarded-For` / `X-Forwarded-Proto`. Enable only behind a reverse proxy you control. |
| `AMIGO_API_TOKEN` | Legacy pre-shared bearer token; accepted alongside session cookies and pairing-issued API tokens. |

## Reverse proxy (nginx, Caddy, Traefik)

Serve amigo behind TLS termination on a subdomain (recommended over a
sub-path). Set `AMIGO_TRUST_PROXY=true` so the pairing flow and rate
limiter see the real client IP.

### nginx

```nginx
server {
    listen 443 ssl http2;
    server_name amigo.example.com;

    location / {
        proxy_pass http://127.0.0.1:1516;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket upgrade for /api/v1/ws
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 3600s;
    }
}
```

### Caddy

```caddy
amigo.example.com {
    reverse_proxy 127.0.0.1:1516 {
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }
}
```

Caddy handles WebSocket upgrades automatically.

### Sub-path deployment

Not recommended — the Web UI bundle assumes it owns `/`. If you must,
strip the prefix in the proxy and set the Vite `base` at build time.
Using a subdomain is always easier.

## Tailscale

Bind amigo to `0.0.0.0:1516` (or to the Tailscale interface IP for
belt-and-braces) and reach it at `http://<name>.<tailnet>.ts.net:1516`.
Nothing special to configure on the amigo side — TOFU is safe because
your tailnet isn't the open internet.

Two caveats:

- **Tailscale Serve** (tls on a tailnet-only hostname) works out of the
  box. Set `AMIGO_TRUST_PROXY=true` if you want the real client IP
  instead of `127.0.0.1` in the pairing UI.
- **Tailscale Funnel** exposes your instance to the public internet. Treat
  it exactly like a VPS — set `AMIGO_SETUP_PIN` so random visitors can't
  claim the admin account before you do.

## Pairing the CLI against a remote server

From any machine that can reach the server:

```sh
amigo-dl login http://amigo.example.com
# Prints a fingerprint like "472-189" and polls for approval.

# In the admin browser: click "Approve" on the pop-up device-approval
# card. The fingerprint displayed in the modal should match the one
# printed by the CLI.

amigo-dl remote list       # confirm the remote was saved
amigo-dl remote use <name> # set as default
```

`remotes.toml` is saved under `~/.config/amigo/` with `0600` permissions.
