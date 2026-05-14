CREATE TABLE IF NOT EXISTS "assets" (
  "id"               INTEGER PRIMARY KEY,
  "path"             TEXT    UNIQUE NOT NULL,
  "guid"             TEXT    UNIQUE,
  "asset_type"       TEXT    CHECK (asset_type IN ('scene', 'prefab', 'asset', 'script', 'unknown')),
  "last_modified_ms" INTEGER NOT NULL DEFAULT 0,
  "last_indexed_ms"  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS "objects" (
  "id"                    INTEGER PRIMARY KEY,
  "asset_id"              INTEGER NOT NULL REFERENCES "assets" ("id") ON DELETE CASCADE,
  "local_id"              TEXT    NOT NULL,
  "class_id"              INTEGER NOT NULL,
  "prefab_instance_id"    INTEGER REFERENCES "prefab_instances" ("object_id"),
  "prefab_source_file_id" TEXT,
  "prefab_source_guid"    TEXT,
  UNIQUE ("asset_id", "local_id")
);

CREATE TABLE IF NOT EXISTS "game_objects" (
  "object_id" INTEGER PRIMARY KEY REFERENCES "objects" ("id") ON DELETE CASCADE,
  "name"      TEXT    NOT NULL DEFAULT '',
  "tag"       TEXT    NOT NULL DEFAULT 'Untagged',
  "layer"     INTEGER NOT NULL DEFAULT 0,
  "is_active" INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS "game_object_components" (
  "game_object_id" INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "component_id"   INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "order_index"    INTEGER NOT NULL DEFAULT 0,
  "enabled"        INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY ("game_object_id", "component_id")
);

CREATE TABLE IF NOT EXISTS "transforms" (
  "object_id"      INTEGER PRIMARY KEY REFERENCES "objects" ("id") ON DELETE CASCADE,
  "game_object_id" INTEGER REFERENCES "objects" ("id"),
  "parent_id"      INTEGER REFERENCES "objects" ("id"),
  "sibling_index"  INTEGER NOT NULL DEFAULT 0,
  "pos_x"          REAL    NOT NULL DEFAULT 0,
  "pos_y"          REAL    NOT NULL DEFAULT 0,
  "pos_z"          REAL    NOT NULL DEFAULT 0,
  "rotation_x"     REAL    NOT NULL DEFAULT 0,
  "rotation_y"     REAL    NOT NULL DEFAULT 0,
  "rotation_z"     REAL    NOT NULL DEFAULT 0,
  "rotation_w"     REAL    NOT NULL DEFAULT 1,
  "scale_x"        REAL    NOT NULL DEFAULT 1,
  "scale_y"        REAL    NOT NULL DEFAULT 1,
  "scale_z"        REAL    NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS "prefab_instances" (
  "object_id"           INTEGER PRIMARY KEY REFERENCES "objects" ("id") ON DELETE CASCADE,
  "source_prefab_guid"  TEXT,
  "source_prefab_id"    INTEGER REFERENCES "assets" ("id"),
  "transform_parent_id" INTEGER REFERENCES "objects" ("id")
);

CREATE TABLE IF NOT EXISTS "object_fields" (
  "object_id" INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "key"       TEXT    NOT NULL,
  "value"     TEXT,
  PRIMARY KEY ("object_id", "key")
);

CREATE TABLE IF NOT EXISTS "prefab_property_overrides" (
  "id"              INTEGER PRIMARY KEY,
  "instance_id"     INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "target_file_id"  TEXT    NOT NULL,
  "target_guid"     TEXT    NOT NULL,
  "property_path"   TEXT    NOT NULL,
  "value"           TEXT,
  "obj_ref_file_id" TEXT,
  "obj_ref_guid"    TEXT
);

CREATE TABLE IF NOT EXISTS "prefab_removals" (
  "instance_id"    INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "target_file_id" TEXT    NOT NULL,
  "target_guid"    TEXT    NOT NULL,
  "removal_type"   TEXT    NOT NULL CHECK (removal_type IN ('component', 'game_object')),
  PRIMARY KEY ("instance_id", "target_file_id", "target_guid")
);

CREATE TABLE IF NOT EXISTS "prefab_additions" (
  "id"              INTEGER PRIMARY KEY,
  "instance_id"     INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "added_object_id" INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "addition_type"   TEXT    NOT NULL CHECK (addition_type IN ('component', 'game_object')),
  "parent_file_id"  TEXT,
  "parent_guid"     TEXT
);

CREATE TABLE IF NOT EXISTS "asset_references" (
  "from_object_id" INTEGER NOT NULL REFERENCES "objects" ("id") ON DELETE CASCADE,
  "field_path"     TEXT    NOT NULL,
  "to_guid"        TEXT    NOT NULL,
  "to_file_id"     TEXT,
  "ref_type"       INTEGER,
  PRIMARY KEY ("from_object_id", "field_path")
);

CREATE INDEX IF NOT EXISTS "idx_assets_guid"             ON "assets"                    ("guid");
CREATE INDEX IF NOT EXISTS "idx_objects_asset_class"     ON "objects"                   ("asset_id", "class_id");
CREATE INDEX IF NOT EXISTS "idx_objects_class"           ON "objects"                   ("class_id");
CREATE INDEX IF NOT EXISTS "idx_objects_prefab_instance" ON "objects"                   ("prefab_instance_id");
CREATE INDEX IF NOT EXISTS "idx_go_name"                 ON "game_objects"              ("name");
CREATE INDEX IF NOT EXISTS "idx_go_tag"                  ON "game_objects"              ("tag");
CREATE INDEX IF NOT EXISTS "idx_go_layer"                ON "game_objects"              ("layer");
CREATE INDEX IF NOT EXISTS "idx_goc_component"           ON "game_object_components"    ("component_id");
CREATE INDEX IF NOT EXISTS "idx_goc_enabled"             ON "game_object_components"    ("enabled");
CREATE INDEX IF NOT EXISTS "idx_transforms_parent"       ON "transforms"                ("parent_id");
CREATE INDEX IF NOT EXISTS "idx_transforms_go"           ON "transforms"                ("game_object_id");
CREATE INDEX IF NOT EXISTS "idx_prefab_source_guid"      ON "prefab_instances"          ("source_prefab_guid");
CREATE INDEX IF NOT EXISTS "idx_object_fields_kv"        ON "object_fields"             ("key", "value");
CREATE INDEX IF NOT EXISTS "idx_ppo_instance"            ON "prefab_property_overrides" ("instance_id");
CREATE INDEX IF NOT EXISTS "idx_ppo_target"              ON "prefab_property_overrides" ("target_guid", "target_file_id");
CREATE INDEX IF NOT EXISTS "idx_additions_instance"      ON "prefab_additions"          ("instance_id");
CREATE INDEX IF NOT EXISTS "idx_refs_to_guid"            ON "asset_references"          ("to_guid");
