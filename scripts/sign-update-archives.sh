#!/usr/bin/env bash
# Sign the CLI and server release archives for the self-updater.
#
# Usage: sign-update-archives.sh <dir>
#
# Writes <archive>.sig next to every amigo-dl-* / amigo-server-* archive in
# <dir>: the lowercase hex of the Ed25519 signature over the archive bytes,
# which `amigo_core::updater::verify_ed25519` checks.
#
# Environment:
#   AMIGO_UPDATE_SIGNING_KEY  32-byte Ed25519 private seed, 64 hex chars (secret)
#   AMIGO_UPDATE_PUBKEY_HEX   matching public key pinned into the binaries
#
# The public key derived from the seed must equal AMIGO_UPDATE_PUBKEY_HEX, so
# a wrong secret fails the release instead of publishing signatures every
# client rejects. Requires OpenSSL 3 (`pkeyutl -rawin`). See
# docs/specs/self-update.md.
set -euo pipefail

dir="${1:?usage: $0 <dir>}"

is_hex64() { [[ "$1" =~ ^[0-9a-fA-F]{64}$ ]]; }
to_hex() { od -An -tx1 -v | tr -d ' \n'; }

seed="${AMIGO_UPDATE_SIGNING_KEY:-}"
pinned="${AMIGO_UPDATE_PUBKEY_HEX:-}"
is_hex64 "$seed" || { echo "AMIGO_UPDATE_SIGNING_KEY must be 64 hex chars" >&2; exit 1; }
is_hex64 "$pinned" || { echo "AMIGO_UPDATE_PUBKEY_HEX must be 64 hex chars" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
chmod 700 "$work"

# PKCS#8 DER for a raw Ed25519 seed: fixed 16-byte prefix + the 32-byte seed.
key="$work/key.der"
printf '%b' "$(printf '302e020100300506032b657004220420%s' "$seed" | sed 's/../\\x&/g')" > "$key"
pub="$work/pub.der"
openssl pkey -inform DER -in "$key" -pubout -outform DER -out "$pub"

derived="$(tail -c 32 "$pub" | to_hex)"
if [[ "$derived" != "${pinned,,}" ]]; then
  echo "AMIGO_UPDATE_SIGNING_KEY does not match AMIGO_UPDATE_PUBKEY_HEX" >&2
  echo "  derived: $derived" >&2
  echo "  pinned:  ${pinned,,}" >&2
  exit 1
fi

shopt -s nullglob
archives=("$dir"/amigo-dl-*.tar.gz "$dir"/amigo-dl-*.zip
          "$dir"/amigo-server-*.tar.gz "$dir"/amigo-server-*.zip)
if [[ ${#archives[@]} -eq 0 ]]; then
  echo "no amigo-dl-* / amigo-server-* archives in $dir" >&2
  exit 1
fi

for archive in "${archives[@]}"; do
  sig="$work/sig.bin"
  openssl pkeyutl -sign -rawin -keyform DER -inkey "$key" -in "$archive" -out "$sig"
  openssl pkeyutl -verify -rawin -pubin -keyform DER -inkey "$pub" \
    -in "$archive" -sigfile "$sig" > /dev/null
  to_hex < "$sig" > "$archive.sig"
  echo "signed $(basename "$archive")"
done
