use std::path::Path;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use crate::unity_data::unity_daemon_server::UnityDaemon;
use crate::unity_data::{IndexProjectRequest, IndexProjectResponse, ReIndexRequest};
use crate::{db, indexer};

pub struct DaemonService;

#[tonic::async_trait]
impl UnityDaemon for DaemonService {
    async fn index_project(
        &self,
        request: Request<IndexProjectRequest>,
    ) -> Result<Response<IndexProjectResponse>, Status> {
        let req = request.into_inner();
        run_index(&req.assets_path, &req.db_path).await
    }

    async fn re_index(
        &self,
        request: Request<ReIndexRequest>,
    ) -> Result<Response<IndexProjectResponse>, Status> {
        let req = request.into_inner();
        run_index(&req.assets_path, &req.db_path).await
    }
}

async fn run_index(assets_path: &str, db_path: &str) -> Result<Response<IndexProjectResponse>, Status> {
    let assets_path = assets_path.to_string();
    let db_path     = db_path.to_string();

    info!(assets_path, db_path, "index request received");

    let result = tokio::task::spawn_blocking(move || {
        let mut conn = db::open(Path::new(&db_path))
            .map_err(|e| format!("failed to open db: {e}"))?;

        indexer::index_project(Path::new(&assets_path), &mut conn)
            .map_err(|e| format!("indexing failed: {e}"))
    })
    .await
    .map_err(|e| Status::internal(format!("task panicked: {e}")))?;

    match result {
        Ok(stats) => {
            if !stats.errors.is_empty() {
                for e in &stats.errors {
                    warn!(error = e.as_str(), "file indexing error");
                }
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
