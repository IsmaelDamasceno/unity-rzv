#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PROTO_DIR="$REPO_ROOT/proto-schema"
OUT_FILE="${1:-$SCRIPT_DIR/schema.pb}"

mkdir -p "$(dirname "$OUT_FILE")"

protoc \
    --proto_path="$PROTO_DIR" \
    --descriptor_set_out="$OUT_FILE" \
    --include_source_info \
    "$PROTO_DIR/schema.proto"

echo "Descriptor written to $OUT_FILE"

uv run sabledocs
