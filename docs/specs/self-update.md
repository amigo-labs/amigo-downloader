# Self-Update (CLI and Server)

**Date**: 2026-10-12
**Status**: Implemented
**Scope**: `crates/core/src/updater.rs`, `crates/server/src/update_api.rs`,
`crates/cli/src/main.rs`, `.github/workflows/release-build.yml`,
`scripts/sign-update-archives.sh`

---

## Summary

`amigo-server` (`POST /api/v1/updates/core`) and `amigo-dl update apply`
replace themselves with the latest GitHub release. Docker images are replaced
by pulling a new image, and the Tauri app has its own updater; neither is
covered here.

Before this spec the path was broken in three ways:

1. Releases ship archives (`amigo-server-linux-x86_64.tar.gz`, `….zip` on
   Windows), but the updater installed the downloaded bytes as the executable,
   which would have replaced the binary with a tarball.
2. `select_asset` always looked for `amigo-server-…`, so `amigo-dl` would have
   replaced itself with the server package.
3. Release builds pin an update key and therefore require a `.sig` asset, but
   the release workflow never produced one, so every update was refused.

Archives can also carry `amigo-plugin-host`, which must be updated together
with the binary that spawns it (#79, SH-7a in `plugin-sandbox-hardening.md`).

---

## Release artifacts

For each target the release carries, per product (`amigo-server`, `amigo-dl`):

| Asset | Content |
|---|---|
| `<product>-<os>-<arch>.tar.gz` (Unix) / `.zip` (Windows) | the product binary, plus `amigo-plugin-host` once it ships |
| `<archive>.sha256` | `sha256sum` line for the archive |
| `<archive>.sig` | lowercase hex of the 64-byte Ed25519 signature over the archive bytes |

`<os>` and `<arch>` are Rust's `std::env::consts::OS` / `ARCH`
(`linux`, `macos`, `windows`; `x86_64`, `aarch64`).

### Signing

The `upload` job of `release-build.yml` runs
`scripts/sign-update-archives.sh artifacts` before publishing. It needs:

- the secret `AMIGO_UPDATE_SIGNING_KEY`: the 32-byte Ed25519 private seed as
  64 hex characters;
- the variable `AMIGO_UPDATE_PUBKEY_HEX`: the matching public key that is
  pinned into the binaries.

The script derives the public key from the seed and fails if it differs from
`AMIGO_UPDATE_PUBKEY_HEX`, so a mismatched secret can never publish
signatures that every client would reject. It signs with OpenSSL 3
(`pkeyutl -rawin`), verifies each signature again, and fails if no archive was
found.

Generating a key pair (offline):

```sh
openssl genpkey -algorithm ed25519 -out update-key.pem
# secret AMIGO_UPDATE_SIGNING_KEY (keep offline, never commit):
openssl pkey -in update-key.pem -outform DER | tail -c 32 | od -An -tx1 -v | tr -d ' \n'
# variable AMIGO_UPDATE_PUBKEY_HEX:
openssl pkey -in update-key.pem -pubout -outform DER | tail -c 32 | od -An -tx1 -v | tr -d ' \n'
```

---

## Update flow

1. `check_for_update(client, repo, product)` reads the latest release and
   selects the asset whose name is exactly
   `<product>-<os>-<arch>.tar.gz` (`.zip` on Windows). `can_self_update` is
   false when no such asset exists.
2. `download_and_verify` checks the download, checksum and signature URLs
   against GitHub's hosts, then verifies the Ed25519 signature (when a key is
   pinned) and the SHA-256 over the archive bytes.
3. `apply_update(archive, product)` extracts into a staging directory next to
   the running executable and installs:
   1. `amigo-plugin-host`, if the archive carries it: the existing file is
      renamed to `amigo-plugin-host.old`, the new one is renamed into place.
   2. The product binary via `self_replace`.
   3. If step 2 fails, step 1 is rolled back.
   4. A stale `amigo-plugin-host.old` from an earlier update is removed on the
      next update (Windows cannot delete the file while an old host runs).
4. The running process keeps its old code. A restart is required, as before.

### Extraction rules

- The format is detected from the magic bytes (gzip or zip), not the name.
- Only entries named exactly `<product>[.exe]` or `amigo-plugin-host[.exe]`
  at the archive root (an optional `./` prefix is allowed) are extracted.
  Every other entry is ignored and never written.
- A whitelisted name that is not a regular file (symlink, directory, …),
  a duplicate, or an entry larger than 512 MiB fails the update.
- The product binary is mandatory; `amigo-plugin-host` is optional so that
  archives from before it shipped still install.
- On Unix the installed files are made executable (`0o755`).

---

## Acceptance criteria

- [x] Asset selection matches the exact archive name of the requesting
  product; `amigo-dl` never selects `amigo-server-…` and vice versa, and
  `.sha256` / `.sig` siblings are never selected.
  Test: `select_asset_picks_the_products_archive`,
  `archive_names_match_the_release_workflow`.
- [x] A tar.gz and a zip archive install the product binary and
  `amigo-plugin-host` with their exact bytes.
  Test: `extracts_binaries_from_tar_gz`, `extracts_binaries_from_zip`.
- [x] Unknown entries (including `../` paths) are never written; a
  whitelisted name that is a symlink, a duplicate entry, a missing product
  binary and a payload that is neither gzip nor zip are errors.
  Test: `unknown_entries_are_ignored_and_never_written`,
  `symlink_with_a_whitelisted_name_is_rejected`,
  `duplicate_entries_are_rejected`, `missing_product_binary_is_an_error`,
  `raw_binary_is_not_an_archive`.
- [x] The plugin host is replaced before the product binary; if replacing the
  product binary fails, the previous plugin host is restored (or the new one
  removed when there was none).
  Test: `install_replaces_plugin_host_then_binary`,
  `failed_binary_replace_restores_the_old_plugin_host`,
  `failed_binary_replace_removes_a_new_plugin_host`.
- [x] The signing script's output verifies with the updater's
  `verify_ed25519`.
  Test: `signatures_from_the_release_script_verify` (fixture produced by the
  script with the test key).
- [x] The signing script signs with a matching key, refuses a seed that does
  not match `AMIGO_UPDATE_PUBKEY_HEX`, and fails when it finds no archive.
  Check: step "Smoke-test the update signing script" in the
  `Release build guard` job of `ci.yml`, with a throwaway key.
- [x] Signature verification uses `verify_strict`, like registry signatures.
