use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::unity_data::unity_daemon_server::UnityDaemon;
use crate::unity_data::{
    AssetInfo, ComponentDetail, ContainerInfo, CreateContainerRequest, CreateContainerResponse,
    FieldMatch, FieldReferenceMatch, FindByComponentRequest, FindByComponentResponse,
    FindByFieldReferenceRequest, FindByFieldReferenceResponse, FindByFieldValueRequest,
    FindByFieldValueResponse, GameObjectMatch, GetGameObjectRequest, GetGameObjectResponse,
    GetSceneHierarchyRequest, GetSceneHierarchyResponse, HierarchyNode, IndexProjectRequest,
    IndexProjectResponse, ListAssetsRequest, ListAssetsResponse, ListContainersRequest,
    ListContainersResponse, ReIndexRequest, StopContainerRequest, StopContainerResponse,
};
use crate::{db, indexer};

// ── Container registry ────────────────────────────────────────────────────────

static CONTAINER_COUNTER: AtomicU64 = AtomicU64::new(1);

struct ContainerState {
    assets_path: String,
    db_path:     String,
    conn:        Arc<Mutex<Connection>>,
}

type Registry = Arc<Mutex<HashMap<String, ContainerState>>>;

fn new_container_id() -> String {
    format!("c{}", CONTAINER_COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn get_conn(registry: &Registry, container_id: &str) -> Result<Arc<Mutex<Connection>>, Status> {
    let map = registry.lock().unwrap();
    match map.get(container_id) {
        Some(c) => Ok(Arc::clone(&c.conn)),
        None    => Err(Status::not_found(format!("container '{container_id}' not found"))),
    }
}

fn get_conn_and_assets(registry: &Registry, container_id: &str) -> Result<(Arc<Mutex<Connection>>, String), Status> {
    let map = registry.lock().unwrap();
    match map.get(container_id) {
        Some(c) => Ok((Arc::clone(&c.conn), c.assets_path.clone())),
        None    => Err(Status::not_found(format!("container '{container_id}' not found"))),
    }
}

// ── Service ───────────────────────────────────────────────────────────────────

pub struct DaemonService {
    registry: Registry,
}

impl DaemonService {
    pub fn new() -> Self {
        Self { registry: Arc::new(Mutex::new(HashMap::new())) }
    }
}

#[tonic::async_trait]
impl UnityDaemon for DaemonService {
    async fn create_container(
        &self,
        request: Request<CreateContainerRequest>,
    ) -> Result<Response<CreateContainerResponse>, Status> {
        let req = request.into_inner();
        let db_path_clone = req.db_path.clone();

        let conn = tokio::task::spawn_blocking(move || {
            db::open(Path::new(&db_path_clone)).map_err(|e| format!("failed to open db: {e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?
        .map_err(|e| Status::internal(e))?;

        let container_id = new_container_id();
        self.registry.lock().unwrap().insert(
            container_id.clone(),
            ContainerState {
                assets_path: req.assets_path,
                db_path:     req.db_path,
                conn:        Arc::new(Mutex::new(conn)),
            },
        );

        info!(container_id = container_id.as_str(), "container created");
        Ok(Response::new(CreateContainerResponse { container_id }))
    }

    async fn list_containers(
        &self,
        _request: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        let map = self.registry.lock().unwrap();
        let containers = map
            .iter()
            .map(|(id, c)| ContainerInfo {
                container_id: id.clone(),
                assets_path:  c.assets_path.clone(),
                db_path:      c.db_path.clone(),
            })
            .collect();
        Ok(Response::new(ListContainersResponse { containers }))
    }

    async fn stop_container(
        &self,
        request: Request<StopContainerRequest>,
    ) -> Result<Response<StopContainerResponse>, Status> {
        let id = request.into_inner().container_id;
        match self.registry.lock().unwrap().remove(&id) {
            Some(_) => {
                info!(container_id = id.as_str(), "container stopped");
                Ok(Response::new(StopContainerResponse {}))
            }
            None => Err(Status::not_found(format!("container '{id}' not found"))),
        }
    }

    async fn index_project(
        &self,
        request: Request<IndexProjectRequest>,
    ) -> Result<Response<IndexProjectResponse>, Status> {
        let id = request.into_inner().container_id;
        let (conn_arc, assets_path) = get_conn_and_assets(&self.registry, &id)?;
        run_index(conn_arc, assets_path).await
    }

    async fn re_index(
        &self,
        request: Request<ReIndexRequest>,
    ) -> Result<Response<IndexProjectResponse>, Status> {
        let id = request.into_inner().container_id;
        let (conn_arc, assets_path) = get_conn_and_assets(&self.registry, &id)?;
        run_index(conn_arc, assets_path).await
    }

    async fn list_assets(
        &self,
        request: Request<ListAssetsRequest>,
    ) -> Result<Response<ListAssetsResponse>, Status> {
        let req = request.into_inner();
        info!(container_id = req.container_id.as_str(), asset_type = req.asset_type.as_str(), path_filter = req.path_filter.as_str(), "list_assets");
        let conn_arc = get_conn(&self.registry, &req.container_id)?;

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
                        path:       r.path,
                        guid:       r.guid.unwrap_or_default(),
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
        info!(container_id = req.container_id.as_str(), scene_path = req.scene_path.as_str(), max_depth = req.max_depth, "get_scene_hierarchy");
        let conn_arc = get_conn(&self.registry, &req.container_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::get_scene_hierarchy(&conn, &req.scene_path, req.max_depth).map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(GetSceneHierarchyResponse {
                nodes: rows
                    .into_iter()
                    .map(|r| HierarchyNode {
                        local_id:      r.local_id,
                        name:          r.name,
                        depth:         r.depth,
                        sibling_index: r.sibling_index,
                        ancestry_path: r.ancestry_path,
                    })
                    .collect(),
            })),
            Err(e) => Err(Status::internal(e)),
        }
    }

    async fn find_by_component(
        &self,
        request: Request<FindByComponentRequest>,
    ) -> Result<Response<FindByComponentResponse>, Status> {
        let req = request.into_inner();
        info!(container_id = req.container_id.as_str(), script_name = req.script_name.as_str(), scene_filter = req.scene_filter.as_str(), "find_by_component");
        let conn_arc = get_conn(&self.registry, &req.container_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_component(&conn, &req.script_name, &req.scene_filter).map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByComponentResponse {
                matches: rows
                    .into_iter()
                    .map(|r| GameObjectMatch {
                        scene_path:    r.scene_path,
                        name:          r.name,
                        local_id:      r.local_id,
                        ancestry_path: r.ancestry_path.unwrap_or_default(),
                        script_path:   r.script_path.unwrap_or_default(),
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
            container_id = req.container_id.as_str(),
            field_key    = req.field_key.as_str(),
            field_value  = req.field_value.as_str(),
            scene_filter = req.scene_filter.as_str(),
            "find_by_field_value"
        );
        let conn_arc = get_conn(&self.registry, &req.container_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_field_value(&conn, &req.field_key, &req.field_value, &req.script_filter, &req.scene_filter)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByFieldValueResponse {
                matches: rows
                    .into_iter()
                    .map(|r| FieldMatch {
                        scene_path:           r.scene_path,
                        game_object_name:     r.game_object_name,
                        game_object_local_id: r.game_object_local_id,
                        script_path:          r.script_path.unwrap_or_default(),
                        field_key:            r.field_key,
                        field_value:          r.field_value,
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
        info!(container_id = req.container_id.as_str(), target_script = req.target_script.as_str(), scene_filter = req.scene_filter.as_str(), "find_by_field_reference");
        let conn_arc = get_conn(&self.registry, &req.container_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            db::find_by_field_reference(&conn, &req.target_script, &req.scene_filter)
                .map_err(|e| format!("{e}"))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok(rows) => Ok(Response::new(FindByFieldReferenceResponse {
                matches: rows
                    .into_iter()
                    .map(|r| FieldReferenceMatch {
                        scene_path:           r.scene_path,
                        game_object_name:     r.game_object_name,
                        game_object_local_id: r.game_object_local_id,
                        script_path:          r.script_path.unwrap_or_default(),
                        field_key:            r.field_key,
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
        info!(container_id = req.container_id.as_str(), scene_path = req.scene_path.as_str(), local_id = req.local_id.as_str(), "get_game_object");
        let conn_arc = get_conn(&self.registry, &req.container_id)?;

        let result = tokio::task::spawn_blocking(move || {
            let conn = conn_arc.lock().unwrap();
            let go    = db::get_game_object(&conn, &req.scene_path, &req.local_id).map_err(|e| format!("{e}"))?;
            let comps = db::get_game_object_components(&conn, &req.scene_path, &req.local_id).map_err(|e| format!("{e}"))?;
            Ok::<_, String>((go, comps))
        })
        .await
        .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

        match result {
            Ok((Some(go), comps)) => Ok(Response::new(GetGameObjectResponse {
                name:      go.name,
                tag:       go.tag,
                layer:     go.layer as i32,
                is_active: go.is_active,
                components: comps
                    .into_iter()
                    .map(|c| ComponentDetail {
                        local_id:    c.local_id,
                        class_id:    c.class_id,
                        script_path: c.script_path.unwrap_or_default(),
                    })
                    .collect(),
            })),
            Ok((None, _)) => Err(Status::not_found("game object not found")),
            Err(e)        => Err(Status::internal(e)),
        }
    }
}

// ── Index handler ─────────────────────────────────────────────────────────────

async fn run_index(
    conn_arc:    Arc<Mutex<Connection>>,
    assets_path: String,
) -> Result<Response<IndexProjectResponse>, Status> {
    info!(assets_path, "index request received");

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = conn_arc.lock().unwrap();
        indexer::index_project(Path::new(&assets_path), &mut conn)
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
                assets_indexed  = stats.assets_indexed,
                objects_indexed = stats.objects_indexed,
                errors          = stats.errors.len(),
                "index complete"
            );
            Ok(Response::new(IndexProjectResponse {
                assets_indexed:  stats.assets_indexed,
                objects_indexed: stats.objects_indexed,
                errors:          stats.errors,
            }))
        }
        Err(msg) => Err(Status::internal(msg)),
    }
}
