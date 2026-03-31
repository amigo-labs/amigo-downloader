#!/usr/bin/env bash
set -euo pipefail

# amigo-downloader — Local Container Build & Start (Docker / Podman)
# Usage: ./scripts/container-start.sh [--build | --rebuild | --stop | --logs]

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_FILE="$REPO_ROOT/docker/docker-compose.yml"
COMPOSE_DEV_FILE="$REPO_ROOT/docker/docker-compose.dev.yml"
IMAGE_NAME="amigo-downloader:local"
DOCKERFILE="$REPO_ROOT/docker/Dockerfile"

# Detect container runtime: prefer docker, fall back to podman
detect_runtime() {
    if command -v docker &>/dev/null; then
        RUNTIME="docker"
        if command -v "docker-compose" &>/dev/null && ! docker compose version &>/dev/null 2>&1; then
            COMPOSE="docker-compose"
        else
            COMPOSE="docker compose"
        fi
    elif command -v podman &>/dev/null; then
        RUNTIME="podman"
        if command -v podman-compose &>/dev/null; then
            COMPOSE="podman-compose"
        else
            COMPOSE="podman compose"
        fi
    else
        echo "Error: Neither docker nor podman found. Install one of them first." >&2
        exit 1
    fi
    echo "Using runtime: $RUNTIME ($COMPOSE)"
}

detect_runtime

usage() {
    echo "Usage: $0 [OPTION]"
    echo ""
    echo "Options:"
    echo "  (none)      Build if needed, then start"
    echo "  --build     Force rebuild and start"
    echo "  --rebuild   Clean rebuild (no cache) and start"
    echo "  --dev       Start web-ui dev server (builds + watches)"
    echo "  --stop      Stop and remove container"
    echo "  --logs      Follow container logs"
    echo "  --status    Show container status"
}

start_dev() {
    echo "Starting web-ui dev server (build + watch)..."
    echo "  Vite Dev:  http://localhost:5173"
    echo ""
    trap '$COMPOSE -f "$COMPOSE_DEV_FILE" down; exit 0' INT TERM
    $COMPOSE -f "$COMPOSE_DEV_FILE" up --build --abort-on-container-exit --remove-orphans
    $COMPOSE -f "$COMPOSE_DEV_FILE" down
}

build_image() {
    local cache_flag="${1:-}"
    echo "Building amigo-downloader image locally..."
    $RUNTIME build $cache_flag -t "$IMAGE_NAME" -f "$DOCKERFILE" "$REPO_ROOT"
}

start_container() {
    echo "Starting amigo-downloader (Ctrl+C to stop and remove)..."
    echo "  Web-UI:       http://localhost:1516"
    echo "  Click'n'Load: http://localhost:9666"
    echo ""
    # Run in foreground; on Ctrl+C, compose down removes the container
    trap '$COMPOSE -f "$COMPOSE_FILE" down; exit 0' INT TERM
    $COMPOSE -f "$COMPOSE_FILE" up --abort-on-container-exit --remove-orphans
    $COMPOSE -f "$COMPOSE_FILE" down
}

case "${1:-}" in
    --build)
        build_image
        start_container
        ;;
    --rebuild)
        build_image "--no-cache"
        start_container
        ;;
    --dev)
        start_dev
        ;;
    --stop)
        echo "Stopping amigo-downloader..."
        $COMPOSE -f "$COMPOSE_FILE" down
        ;;
    --logs)
        $COMPOSE -f "$COMPOSE_FILE" logs -f
        ;;
    --status)
        $COMPOSE -f "$COMPOSE_FILE" ps
        ;;
    --help|-h)
        usage
        ;;
    *)
        # Build only if image doesn't exist
        if ! $RUNTIME image inspect "$IMAGE_NAME" &>/dev/null; then
            build_image
        fi
        start_container
        ;;
esac
