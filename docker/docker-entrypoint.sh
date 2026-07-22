#!/bin/bash
# amigo-downloader container entrypoint.
#
# Ensures the config and download volumes are writable by the runtime user
# before starting the server, instead of failing later with confusing errors
# when a host volume is owned by something other than the container user.
#
# Supports linuxserver.io-style PUID/PGID overrides: set them to match the
# owner of your host-mounted volumes and the container adopts those IDs.
set -euo pipefail

PUID="${PUID:-1000}"
PGID="${PGID:-1000}"

CONFIG_DIR="${AMIGO_CONFIG_DIR:-/config}"
DOWNLOAD_DIR="${AMIGO_DOWNLOAD_DIR:-/downloads}"

docs_hint() {
    echo "amigo: see the volume-permissions section of the docs" \
        "(https://github.com/amigo-labs/amigo-downloader) — set PUID/PGID to" \
        "match your host volume owner, or run the container as root." >&2
}

if [ "$(id -u)" = "0" ]; then
    # Running as root: align the amigo user/group with the requested IDs, take
    # ownership of the volumes, then drop privileges via gosu.
    current_gid="$(getent group amigo | cut -d: -f3)"
    if [ "$PGID" != "$current_gid" ]; then
        groupmod -o -g "$PGID" amigo
    fi
    current_uid="$(id -u amigo)"
    if [ "$PUID" != "$current_uid" ]; then
        usermod -o -u "$PUID" amigo
    fi

    mkdir -p "$CONFIG_DIR" "$DOWNLOAD_DIR"
    # /config is small (database + config) so a recursive chown is cheap and
    # correct. Only the /downloads mount point itself is chowned — a recursive
    # chown over a large existing library could take a very long time; new
    # files are created as the amigo user regardless.
    chown -R amigo:amigo "$CONFIG_DIR"
    chown amigo:amigo "$DOWNLOAD_DIR"

    if ! gosu amigo test -w "$DOWNLOAD_DIR"; then
        echo "amigo: download dir '$DOWNLOAD_DIR' is not writable by UID $PUID." >&2
        docs_hint
        exit 1
    fi

    echo "amigo: starting as UID $PUID / GID $PGID"
    exec gosu amigo "$@"
fi

# Not root (image started with --user): we cannot fix ownership, only verify
# and fail loudly with a pointer to the docs.
for dir in "$CONFIG_DIR" "$DOWNLOAD_DIR"; do
    mkdir -p "$dir" 2>/dev/null || true
    if [ ! -w "$dir" ]; then
        echo "amigo: '$dir' is not writable by UID $(id -u)." >&2
        docs_hint
        exit 1
    fi
done

exec "$@"
