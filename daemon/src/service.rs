use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::unity_data::unity_daemon_server::UnityDaemon;
use crate::unity_data::{
    AssetInfo, ComponentDetail, CreateWorkspaceRequest, CreateWorkspaceResponse,
    DeleteWorkspaceRequest, DeleteWorkspaceResponse, FieldMatch, FieldReferenceMatch,
    FindByComponentRequest, FindByComponentResponse, FindByFieldReferenceRequest,
    FindByFieldReferenceResponse, FindByFieldValueRequest, FindByFieldValueResponse,
    FindByLocalIdRequest, FindByLocalIdResponse,
    GameObjectMatch, GetGameObjectRequest, GetGameObjectResponse, GetSceneHierarchyRequest,
    GetSceneHierarchyResponse, HierarchyNode, IndexProjectRequest, IndexProjectResponse,
    ListAssetsRequest, ListAssetsResponse, ListWorkspacesRequest, ListWorkspacesResponse, WorkspaceInfo,
};
use crate::{db, indexer};

// ── Workspace registry ───────────────────────────────────────────────────────

static WORKSPACE_COUNTER: AtomicU64 = AtomicU64::new(1);

struct WorkspaceState {
    project_path: String,
    db_path: String,
    conn: Arc<Mutex<Connection>>,
}

type Registry = Arc<Mutex<HashMap<String, WorkspaceState>>>;

fn new_workspace_id() -> String {
    format!("w{}", WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn get_conn(registry: &Registry, workspace_id: &str) -> Result<Arc<Mutex<Connection>>, Status> {
    let map = registry.lock().unwrap();
    match map.get(workspace_id) {
        Some(w) => Ok(Arc::clone(&w.conn)),
        None => Err(Status::not_found(format!(
            "workspace '{workspace_id}' not found"
        ))),
    }
}

fn get_conn_and_proj_path(
    registry: &Registry,
    workspace_id: &str,
) -> Result<(Arc<Mutex<Connection>>, String), Status> {
    let map = registry.lock().unwrap();
    match map.get(workspace_id) {
        Some(w) => Ok((Arc::clone(&w.conn), w.project_path.clone())),
        None => Err(Status::not_found(format!(
            "workspace '{workspace_id}' not found"
        ))),
    }
}

// ── Service ───────────────────────────────────────────────────────────────────

pub struct DaemonService {
    registry: Registry,
}

impl DaemonService {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tonic::async_trait]
impl UnityDaemon for DaemonService {
    async fn create_workspace(
        &self,
        request: Request<CreateWorkspaceRequest>,
    ) -> Result<Response<CreateWorkspaceResponse>, Status> {
        let req = request.into_inner();
        let db_path = req
            .db_path
            .unwrap_or_else(|| format!("{}/unity-rzv.db", req.path));

        let db_path_clone = db_path.clone();
        let conn = tokio::task::spawn_blocking(move || {
            db::open(Path::new(&db_path_clone)).map_err(|e| format!("failed to open db: {e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?
        .map_err(|e| Status::internal(e))?;

        let workspace_id = new_workspace_id();

        let conn_arc = {
            let mut dict = self.registry.lock().unwrap();
            dict.insert(
                workspace_id.clone(),
                WorkspaceState {
                    project_path: req.path.clone(),
                    db_path,
                    conn: Arc::new(Mutex::new(conn)),
                },
            );
            Arc::clone(&dict.get(&workspace_id).unwrap().conn)
        };

        info!(workspace_id = workspace_id.as_str(), "workspace created");

        match run_index(conn_arc, req.path).await {
            Ok(index) => {
                Ok(Response::new(CreateWorkspaceResponse { workspace_id, indexing_succeeded: true, index: Some(index.into_inner()) }))
            }
            Err(e) => {
                warn!("Initial index failed: {e}");
                Ok(Response::new(CreateWorkspaceResponse { workspace_id, indexing_succeeded: false, index: None }))
            }
        }
    }

    async fn list_workspaces(
        &self,
        _request: Request<ListWorkspacesRequest>,
    ) -> Result<Response<ListWorkspacesResponse>, Status> {
        let map = self.registry.lock().unwrap();
        let workspaces = map
            .iter()
            .map(|(id, w)| WorkspaceInfo {
                workspace_id: id.clone(),
                project_path: w.project_path.clone(),
                db_path: w.db_path.clone(),
            })
            .collect();
        Ok(Response::new(ListWorkspacesResponse { workspaces }))
    }

    async fn delete_workspace(
        &self,
        request: Request<DeleteWorkspaceRequest>,
    ) -> Result<Response<DeleteWorkspaceResponse>, Status> {
        let id = request.into_inner().workspace_id;
        match self.registry.lock().unwrap().remove(&id) {
            Some(_) => {
                info!(workspace_id = id.as_str(), "workspace deleted");
                Ok(Response::new(DeleteWorkspaceResponse {}))
            }
            None => Err(Status::not_found(format!("workspace '{id}' not found"))),
        }
    }

    async fn index_project(
        &self,
        request: Request<IndexProjectRequest>,
    ) -> Result<Response<IndexProjectResponse>, Status> {
        let id = request.into_inner().workspace_id;
        let (conn_arc, project_path) = get_conn_and_proj_path(&self.registry, &id)?;
        run_index(conn_arc, project_path).await
    }

    async fn list_assets(
        &self,
        request: Request<ListAssetsRequest>,
    ) -> Result<Response<ListAssetsResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            asset_type = req.asset_type.as_str(),
            path_filter = req.path_filter.as_str(),
            "list_assets"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::list_assets(&conn, &req.asset_type, &req.path_filter).map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(ListAssetsResponse {
                assets: rows
                    .into_iter()
                    .map(|r| AssetInfo {
                        path: r.path,
                        guid: r.guid.unwrap_or_default(),
                        asset_type: r.asset_type,
                    })
                    .collect(),
            })),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn get_scene_hierarchy(
        &self,
        request: Request<GetSceneHierarchyRequest>,
    ) -> Result<Response<GetSceneHierarchyResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            scene_path = req.scene_path.as_str(),
            max_depth = req.max_depth,
            "get_scene_hierarchy"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::get_scene_hierarchy(&conn, &req.scene_path, req.max_depth, &req.exclude_scripts)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => {
                if req.as_string {
                    let tree = rows
                        .iter()
                        .map(|r| format!("{}{}", "  ".repeat(r.depth as usize), r.name))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(Response::new(GetSceneHierarchyResponse { nodes: vec![], tree }))
                } else {
                    Ok(Response::new(GetSceneHierarchyResponse {
                        nodes: rows
                            .into_iter()
                            .map(|r| HierarchyNode {
                                local_id: r.local_id,
                                name: r.name,
                                depth: r.depth,
                                sibling_index: r.sibling_index,
                                ancestry_path: r.ancestry_path,
                            })
                            .collect(),
                        tree: String::new(),
                    }))
                }
            }
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn find_by_component(
        &self,
        request: Request<FindByComponentRequest>,
    ) -> Result<Response<FindByComponentResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            script_name = req.script_name.as_str(),
            scene_filter = req.scene_filter.as_str(),
            "find_by_component"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_component(&conn, &req.script_name, &req.scene_filter)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByComponentResponse {
                matches: rows
                    .into_iter()
                    .map(|r| GameObjectMatch {
                        scene_path: r.scene_path,
                        name: r.name,
                        local_id: r.local_id,
                        ancestry_path: r.ancestry_path.unwrap_or_default(),
                        script_path: r.script_path.unwrap_or_default(),
                    })
                    .collect(),
            })),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn find_by_field_value(
        &self,
        request: Request<FindByFieldValueRequest>,
    ) -> Result<Response<FindByFieldValueResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            field_key = req.field_key.as_str(),
            field_value = req.field_value.as_str(),
            scene_filter = req.scene_filter.as_str(),
            "find_by_field_value"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_field_value(
                &conn,
                &req.field_key,
                &req.field_value,
                &req.script_filter,
                &req.scene_filter,
            )
            .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByFieldValueResponse {
                matches: rows
                    .into_iter()
                    .map(|r| FieldMatch {
                        scene_path: r.scene_path,
                        game_object_name: r.game_object_name,
                        game_object_local_id: r.game_object_local_id,
                        script_path: r.script_path.unwrap_or_default(),
                        field_key: r.field_key,
                        field_value: r.field_value,
                    })
                    .collect(),
            })),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn find_by_field_reference(
        &self,
        request: Request<FindByFieldReferenceRequest>,
    ) -> Result<Response<FindByFieldReferenceResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            target_asset = req.target_asset.as_str(),
            scene_filter = req.scene_filter.as_str(),
            "find_by_field_reference"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_field_reference(&conn, &req.target_asset, &req.scene_filter)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByFieldReferenceResponse {
                matches: rows
                    .into_iter()
                    .map(|r| {
                        let component_label = r.script_path.as_deref()
                            .and_then(|p| Path::new(p).file_stem()?.to_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("class:{}", r.class_id));
                        let ancestry = r.ancestry_path.as_deref().unwrap_or(&r.game_object_name);
                        let ancestry_path  = ancestry
                            .split('\x1F')
                            .chain([component_label.as_str(), r.field_key.as_str()])
                            .collect::<Vec<_>>()
                            .join("\x1F");
                        FieldReferenceMatch {
                            scene_path: r.scene_path,
                            game_object_name: r.game_object_name,
                            game_object_local_id: r.game_object_local_id,
                            script_path: r.script_path.unwrap_or_default(),
                            field_key: r.field_key,
                            ancestry_path ,
                        }
                    })
                    .collect(),
            })),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn find_by_local_id(
        &self,
        request: Request<FindByLocalIdRequest>,
    ) -> Result<Response<FindByLocalIdResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            local_id = req.local_id.as_str(),
            scene_filter = req.scene_filter.as_str(),
            "find_by_local_id"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_local_id(&conn, &req.local_id, &req.scene_filter)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByLocalIdResponse {
                matches: rows
                    .into_iter()
                    .map(|r| GameObjectMatch {
                        scene_path: r.scene_path,
                        name: r.name,
                        local_id: r.local_id,
                        ancestry_path: r.ancestry_path.unwrap_or_default(),
                        script_path: r.script_path.unwrap_or_default(),
                    })
                    .collect(),
            })),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn get_game_object(
        &self,
        request: Request<GetGameObjectRequest>,
    ) -> Result<Response<GetGameObjectResponse>, Status> {
        let req = request.into_inner();
        info!(
            workspace_id = req.workspace_id.as_str(),
            scene_path = req.scene_path.as_str(),
            local_id = req.local_id.as_str(),
            "get_game_object"
        );
        let conn_arc = get_conn(&self.registry, &req.workspace_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            let go = db::get_game_object(&conn, &req.scene_path, &req.local_id)
                .map_err(|e| format!("{e}"))?;
            let comps = db::get_game_object_components(&conn, &req.scene_path, &req.local_id)
                .map_err(|e| format!("{e}"))?;
            Ok::<_, String>((go, comps))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok((Some(go), comps)) => Ok(Response::new(GetGameObjectResponse {
                name: go.name,
                tag: go.tag,
                layer: go.layer as i32,
                is_active: go.is_active,
                components: comps
                    .into_iter()
                    .map(|c| ComponentDetail {
                        local_id: c.local_id,
                        class_id: c.class_id,
                        script_path: c.script_path.unwrap_or_default(),
                    })
                    .collect(),
            })),
            Ok((None, _)) => Err(Status::not_found("game object not found")),
            Err(e) => Err(Status::internal(e)),
        }
    }
}

// ── Index handler ─────────────────────────────────────────────────────────────

async fn run_index(
    conn_arc: Arc<Mutex<Connection>>,
    project_path: String,
) -> Result<Response<IndexProjectResponse>, Status> {
    info!(project_path, "index request received");

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = conn_arc.lock().unwrap();
        indexer::index_project(Path::new(&project_path), &mut conn)
            .map_err(|e| format!("indexing failed: {e}"))
    })
    .await
    .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

    match result {
        Ok(stats) => {
            for e in &stats.errors {
                warn!(error = e.as_str(), "file indexing error");
            }
            info!(
                assets_indexed = stats.assets_indexed,
                objects_indexed = stats.objects_indexed,
                errors = stats.errors.len(),
                "index complete"
            );
            Ok(Response::new(IndexProjectResponse {
                assets_indexed: stats.assets_indexed,
                objects_indexed: stats.objects_indexed,
                errors: stats.errors,
            }))
        }
        Err(msg) => Err(Status::internal(msg)),
    }
}
