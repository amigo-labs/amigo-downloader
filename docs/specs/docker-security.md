# Docker Security Hardening

**Date**: 2026-04-02
**Status**: Draft
**Scope**: Harden Docker deployment — Dockerfile, docker-compose.yml, Dockerfile.dev

---

## Summary

A security audit of the Docker deployment identified five issues spanning container health observability, privilege escalation surface, build reproducibility, credential hygiene, and dev build correctness. None are critical vulnerabilities in isolation, but together they represent a weak deployment posture that would fail most container security scanners (Trivy, Dockle, etc.).

---

## Issues

### DS-1. No Health Checks

**Problem**: Neither `docker/Dockerfile` nor `docker/docker-compose.yml` defines a `HEALTHCHECK`. Container orchestrators (Docker Swarm, Kubernetes via liveness probes, Compose restart policies) cannot distinguish a running-but-hung process from a healthy one. The container will stay in "running" state even if `amigo-server` has deadlocked or crashed its listener.

**Fix**: Add a `HEALTHCHECK` instruction to the production Dockerfile that hits the existing `GET /api/v1/status` endpoint. Mirror this in docker-compose.yml via the `healthcheck:` key for users who build from Compose directly.

**Implementation notes**:
- Use `curl` or a minimal static binary (e.g., `wget` from busybox) to avoid adding a large dependency to the slim runtime image. `curl` is not present in `debian:bookworm-slim` by default — either install it or use `wget` which is smaller.
- Interval: 30s, timeout: 5s, retries: 3, start_period: 10s are reasonable defaults.
- The health endpoint should return HTTP 200 when the server is ready to accept downloads.

---

### DS-2. Running as Root

**Problem**: The production Dockerfile has no `USER` instruction. The `amigo-server` process runs as UID 0 inside the container. If the application is compromised (e.g., via a malicious plugin or a vulnerability in a dependency), the attacker has root access to the entire container filesystem, can install packages, and potentially escape to the host if the container runtime has misconfigured capabilities.

**Fix**: Create a non-root user and group in the runtime stage and switch to it before `ENTRYPOINT`.

**Implementation notes**:
- Add `RUN groupadd -r amigo && useradd -r -g amigo -d /config -s /sbin/nologin amigo` in the runtime stage.
- `chown` the `/config`, `/downloads`, `/etc/amigo/plugins`, and `/etc/amigo/locales` directories to `amigo:amigo`.
- Add `USER amigo` before `ENTRYPOINT`.
- Document that host-mounted volumes (`./config`, `./downloads`) must be writable by the container UID. Provide the UID in the Dockerfile as a label or comment (e.g., UID 1000 or a fixed value).
- Ensure the Click'n'Load port 9666 (>1024) does not require root. Port 1516 is also >1024, so no capability issues.

---

### DS-3. Non-Pinned Base Image Versions

**Problem**: The Dockerfile uses `node:22-alpine`, `rust:1-bookworm`, and `debian:bookworm-slim` as base images. These floating tags resolve to different images over time as patch versions are released. This means:
- Builds are not reproducible — the same Dockerfile may produce different images on different days.
- A broken upstream image can silently break the build.
- Supply chain attacks on popular tags are a known threat vector.

**Fix**: Pin base images to specific digest hashes (preferred) or at minimum to full patch versions (e.g., `node:22.14.0-alpine3.21`).

**Implementation notes**:
- Full digest pinning (`FROM node:22-alpine@sha256:abc123...`) provides the strongest guarantee but requires manual updates. This is the recommended approach.
- As a pragmatic middle ground, pin to full patch versions and document a process for bumping them (e.g., Dependabot, Renovate, or a manual quarterly review).
- Apply to all three stages: UI builder, Rust builder, and runtime.
- Apply the same pinning to `Dockerfile.dev` (`node:22-alpine`).

---

### DS-4. Default Credentials in docker-compose.yml

**Problem**: `docker-compose.yml` contains `AMIGO_API_KEY=changeme` in plaintext. Users who deploy without changing this value expose their instance with a known default key. Automated scanners and bots actively probe for default credentials.

**Fix**: Remove the hardcoded default and require explicit configuration.

**Implementation notes**:
- Replace the hardcoded value with a reference to an environment variable or `.env` file: `AMIGO_API_KEY=${AMIGO_API_KEY:?Set AMIGO_API_KEY in .env or environment}`.
- Add a `.env.example` file in `docker/` with `AMIGO_API_KEY=` (empty) and a comment explaining how to generate a secure key.
- Add `.env` to `.dockerignore` and `.gitignore` if not already present.
- The server should refuse to start (or log a prominent warning) if the API key is empty or equals "changeme". This is a server-side change but should be noted in the spec.
- Alternatively, auto-generate a random API key on first run if none is provided, and print it to stdout/logs.

---

### DS-5. Dev Dockerfile Build Order

**Problem**: `Dockerfile.dev` runs `npm run build` in its `ENTRYPOINT`, but the source code is not available at build time — it is volume-mounted at runtime. The `ENTRYPOINT` command (`npm run build && npm run dev -- --host 0.0.0.0`) will fail or use stale code because:
- `npm ci` runs at build time against only `package.json`/`package-lock.json` (correct for deps).
- But `npm run build` at container start requires the full `web-ui/src/` tree, which only exists if the volume mount is correctly configured.
- If the volume is not mounted (or mounted to the wrong path), the build fails silently or with a confusing error.

**Fix**: Remove `npm run build` from the entrypoint. The dev container should only run the Vite dev server, which handles incremental builds and HMR. A production build has no place in a dev workflow.

**Implementation notes**:
- Change `ENTRYPOINT` to `["npm", "run", "dev", "--", "--host", "0.0.0.0"]`.
- Ensure `docker-compose` (or a `docker-compose.dev.yml`) mounts the full `web-ui/` directory, not just `src/`.
- Add a comment in the Dockerfile explaining that source code is expected via volume mount.
- Consider adding a `CMD` that can be overridden for one-off builds: `CMD ["npm", "run", "dev", "--", "--host", "0.0.0.0"]` with `ENTRYPOINT ["sh", "-c"]` or similar.

---

## Acceptance Criteria

| ID | Criterion | How to verify |
|---|---|---|
| AC-1 | Production container reports healthy/unhealthy status | `docker inspect --format='{{.State.Health.Status}}' amigo-downloader` returns `healthy` when server is up and `unhealthy` when server is stopped or unresponsive |
| AC-2 | Production container runs as non-root user | `docker exec amigo-downloader whoami` returns a non-root username (e.g., `amigo`) |
| AC-3 | All base images are pinned to specific versions or digests | `grep -E '^FROM' docker/Dockerfile docker/Dockerfile.dev` shows full version tags or `@sha256:` digests — no floating tags like `node:22-alpine` |
| AC-4 | No default credentials in docker-compose.yml | `grep -i 'changeme' docker/docker-compose.yml` returns no matches; starting without `AMIGO_API_KEY` set produces an error or auto-generates a key |
| AC-5 | Dev container starts successfully with volume-mounted source | `docker compose -f docker-compose.dev.yml up` starts Vite dev server without errors when `web-ui/` is mounted; no `npm run build` in entrypoint |
| AC-6 | Dockle scan passes with no FATAL or WARN findings | `dockle amigo-downloader:local` reports CIS-DI-0001 (non-root), CIS-DI-0006 (healthcheck), DKL-DI-0006 (pinned tags) as PASS |

---

## Out of Scope

- Network policies and firewall rules (host-level concern, not Dockerfile)
- Image signing and Notary/Cosign (deferred to CI/CD spec)
- Read-only root filesystem (`--read-only` flag) — worth pursuing later but requires audit of all write paths first
- Secrets management integration (Vault, Docker secrets) — the `.env` approach is sufficient for now

---

## References

- [Docker Best Practices](https://docs.docker.com/develop/develop-images/dockerfile_best-practices/)
- [CIS Docker Benchmark](https://www.cisecurity.org/benchmark/docker)
- [Dockle container linter](https://github.com/goodwithtech/dockle)
