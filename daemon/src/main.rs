use memchr::memmem;
use std::str;
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::EnvFilter;

use unity_data::unity_daemon_server::UnityDaemonServer;

mod block_mapper;
mod db;
mod indexer;
mod parse;
mod service;
mod types;

pub mod unity_data {
    include!(concat!(env!("OUT_DIR"), "/unity_tool.rs"));
}

// ── Shared parsing primitives ─────────────────────────────────────────────────
// Kept in the crate root so block_mapper can reach them via `crate::`.

pub struct Finders {
    pub sep:       memmem::Finder<'static>,
    pub whitespace: memmem::Finder<'static>,
    pub ampersand: memmem::Finder<'static>,
    pub line_feed: memmem::Finder<'static>,
}

impl Finders {
    pub fn new() -> Self {
        Self {
            sep:       memmem::Finder::new(b"--- !u!"),
            whitespace: memmem::Finder::new(b" "),
            ampersand: memmem::Finder::new(b"&"),
            line_feed: memmem::Finder::new(b"\n"),
        }
    }
}

/// Parses `--- !u!<class_id> &<local_id>[ stripped]`.
/// Returns `(class_id, local_id, is_stripped, body_start)`.
pub fn parse_header<'a>(
    data:    &'a [u8],
    offset:  usize,
    finders: &Finders,
) -> anyhow::Result<(&'a str, &'a str, bool, usize)> {
    let class_id_end = finders.whitespace.find(&data[offset..])
        .ok_or_else(|| anyhow::anyhow!("no space after class ID at offset {offset}"))?;
    let class_id = str::from_utf8(&data[offset..offset + class_id_end])?;

    let header_line_end = finders.line_feed.find(&data[offset..])
        .ok_or_else(|| anyhow::anyhow!("no newline at offset {offset}"))?;
    let header_line = &data[offset..offset + header_line_end];

    let ampersand_pos = finders.ampersand.find(header_line)
        .ok_or_else(|| anyhow::anyhow!("no '&' on header line at offset {offset}"))?;
    let id_start = offset + ampersand_pos + finders.ampersand.needle().len();

    let id_end = finders.line_feed.find(&data[id_start..])
        .ok_or_else(|| anyhow::anyhow!("no newline after local ID at offset {id_start}"))?;
    let raw = str::from_utf8(&data[id_start..id_start + id_end])?;

    let (local_id, is_stripped) = match raw.strip_suffix(" stripped") {
        Some(id) => (id, true),
        None     => (raw, false),
    };

    Ok((class_id, local_id, is_stripped, id_start + id_end + 1))
}

/// Returns the absolute byte position of the next `--- !u!` separator after
/// `from`, or `data.len()` if none exists.
pub fn find_next_block(data: &[u8], from: usize, finders: &Finders) -> usize {
    finders.sep.find(&data[from..]).map(|p| from + p).unwrap_or(data.len())
}

// ── Server startup ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("UNITY_LOG")
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let addr: std::net::SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:50051".to_string())
        .parse()?;

    info!("unity-rzv daemon listening on {addr}");

    Server::builder()
        .add_service(UnityDaemonServer::new(service::DaemonService::new()))
        .serve(addr)
        .await?;

    Ok(())
}
