# unity-rzv

Mono repo for unity-rzv — tooling for indexing and querying Unity projects without loading the editor.

| Directory | Description |
|-----------|-------------|
| `daemon/` | Rust gRPC daemon |
| `proto-schema/` | gRPC service definition (`schema.proto`) |
| `proto-docs/` | Generated API documentation |

## What it does

Unity project files (`.unity`, `.prefab`, `.asset`) are serialized YAML. unity-rzv parses that YAML, extracts the object graph, and stores it in a structured SQLite database — making it queryable without loading the Unity Editor.

Once a workspace is created and indexed, you can:

- List all assets by type or path
- Traverse scene hierarchies
- Find GameObjects by attached component/script
- Search component field values (e.g. find all objects with a specific method name on a UnityEvent)
- Find cross-asset field references

## Concepts

**Workspace** — a registered session pointing at a Unity project on disk. Holds a path to the project's Assets folder and a path to the SQLite database. Multiple workspaces can exist simultaneously.

**IndexProject** — scans the workspace's project path and populates the database. Subsequent calls are incremental (files unchanged since last index are skipped by modification time).

## Running the daemon

```bash
cd daemon
cargo build --release
./target/release/unity-rzv-daemon            # listens on 127.0.0.1:50051
./target/release/unity-rzv-daemon 0.0.0.0:9090  # custom address
```

Log level is controlled via the `UNITY_LOG` environment variable (default: `info`):

```bash
UNITY_LOG=debug ./target/release/unity-rzv-daemon
```

## API

The gRPC API is defined in [`proto-schema/schema.proto`](proto-schema/schema.proto). Connect with any gRPC client.

For full API documentation, see [Generating API docs](#generating-api-docs) below.

## Indexing scope

- `Assets/` — fully indexed
- `Library/PackageCache/com.unity.*` — excluded (Unity built-in packages)
- Other `PackageCache` entries (third-party packages) — indexed
- Audio files, font assets (TMP) — registered but not object-indexed
- Numeric-only field values — not stored (no search value)

## Database

Stored at `<project_path>/unity-rzv.db` by default, or at a custom path passed in `CreateWorkspaceRequest.db_path`. Typical size for a mid-size project is 25–40 MB.

## Generating API docs

```bash
cd proto-docs
./gen-descriptor.sh        # generates schema.pb with source info
uv run sabledocs           # generates HTML docs
```

Requires `protoc` and `uv` with `sabledocs` installed (`uv add --dev sabledocs`).
