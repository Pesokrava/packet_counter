//! Userspace eBPF loader + Axum HTTP server.
//!
//! Responsibilities:
//!   1. Load the compiled XDP program and attach it to a network interface.
//!   2. Run a background task that periodically reads `PACKET_COUNTS` from
//!      the BPF map and caches the aggregated stats behind an `Arc<RwLock<>>`.
//!   3. Serve an Axum HTTP API on `0.0.0.0:3001`:
//!        GET /api/health  — liveness probe
//!        GET /api/stats   — JSON array of per-port packet counts
//!   4. Optionally serve a pre-built React SPA from a runtime `--static-dir` path.
//!   5. Shut down cleanly on Ctrl-C.

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context as _;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use aya::{
    maps::PerCpuHashMap,
    programs::{Xdp, XdpFlags},
    Ebpf,
};
use aya_log::EbpfLogger;
use clap::Parser;
use log::{info, warn};
use packet_counter_common::{PortKey, PROTO_TCP, PROTO_UDP};
use serde::Serialize;
use tokio::{signal, sync::RwLock, time};
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// eBPF packet counter — attaches an XDP program to a network interface,
/// counts packets per destination port (TCP + UDP), and exposes a REST API.
///
/// Communication with the eBPF program happens through a `PerCpuHashMap`
/// BPF map named `PACKET_COUNTS`.  A background task reads the map every
/// second and caches aggregated stats.  The Axum HTTP server serves the
/// cached stats via `GET /api/stats`.
#[derive(Debug, Parser)]
struct Opt {
    /// Network interface to attach to (e.g. eth0, lo, ens3).
    #[clap(short, long, default_value = "eth0")]
    iface: String,

    /// Attach using SKB (generic) mode instead of native XDP mode.
    ///
    /// Native mode (default) runs before the kernel networking stack — lowest
    /// latency. Use this flag when the driver does not support native XDP
    /// (e.g. virtual or container interfaces).
    #[clap(long)]
    skb_mode: bool,

    /// How often (in seconds) to refresh the stats cache from the BPF map.
    #[clap(long, default_value = "1")]
    interval: u64,

    /// Address the HTTP server listens on.
    #[clap(long, default_value = "0.0.0.0:3001")]
    listen: SocketAddr,

    /// Optional path to a pre-built React SPA directory (e.g. `web/dist`).
    ///
    /// When provided, the server mounts `tower_http::services::ServeDir` at `/`
    /// with an SPA fallback to `index.html` so that React Router works correctly.
    /// All `/api/*` routes still take precedence.
    ///
    /// When omitted, the binary operates as a pure API server — no change to
    /// existing behaviour.
    #[clap(long)]
    static_dir: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// A single row returned by `GET /api/stats`.
#[derive(Debug, Clone, Serialize)]
pub struct StatEntry {
    /// Human-readable protocol name: `"tcp"` or `"udp"`.
    pub protocol: String,
    /// Destination port number (host byte order).
    pub port: u16,
    /// Total packet count (sum across all CPUs).
    pub count: u64,
}

/// Stats snapshot shared between the background refresh task and the HTTP
/// handlers via `Arc<RwLock<>>`.
type SharedStats = Arc<RwLock<Vec<StatEntry>>>;

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    env_logger::init();

    // -----------------------------------------------------------------------
    // Load the compiled eBPF bytecode embedded at build time.
    // -----------------------------------------------------------------------
    let mut bpf = Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/packet-counter-ebpf"
    )))?;

    // -----------------------------------------------------------------------
    // Wire up aya-log so that aya_log_ebpf messages are forwarded to log.
    // -----------------------------------------------------------------------
    if let Err(e) = EbpfLogger::init(&mut bpf) {
        warn!("Failed to initialise eBPF logger: {}", e);
    }

    // -----------------------------------------------------------------------
    // Attach the XDP program to the requested interface.
    // -----------------------------------------------------------------------
    let program: &mut Xdp = bpf
        .program_mut("packet_counter")
        .context("BPF program 'packet_counter' not found in object file")?
        .try_into()?;

    program.load()?;

    let xdp_flags = if opt.skb_mode {
        warn!(
            "Attaching in SKB (generic) mode — worse performance than native XDP. \
             Use only when the driver does not support native XDP."
        );
        XdpFlags::SKB_MODE
    } else {
        XdpFlags::default()
    };

    program.attach(&opt.iface, xdp_flags).with_context(|| {
        format!(
            "Failed to attach XDP program to '{}' in {} mode. \
             If the driver does not support native XDP, retry with --skb-mode.",
            opt.iface,
            if opt.skb_mode { "SKB" } else { "native" }
        )
    })?;

    info!(
        "XDP program attached to '{}'. Refreshing stats every {}s.",
        opt.iface, opt.interval
    );

    // -----------------------------------------------------------------------
    // Shared stats cache — written by the background task, read by HTTP
    // handlers.
    // -----------------------------------------------------------------------
    let shared_stats: SharedStats = Arc::new(RwLock::new(Vec::new()));

    // -----------------------------------------------------------------------
    // Background task: periodically reads the BPF map and updates the cache.
    // -----------------------------------------------------------------------
    let stats_writer = Arc::clone(&shared_stats);
    let refresh_interval = Duration::from_secs(opt.interval);

    tokio::spawn(async move {
        let mut ticker = time::interval(refresh_interval);
        loop {
            ticker.tick().await;
            let entries = read_stats(&bpf);
            *stats_writer.write().await = entries;
        }
    });

    // -----------------------------------------------------------------------
    // Axum HTTP server
    // -----------------------------------------------------------------------
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes always take precedence.
    let api_router = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/stats", get(stats_handler))
        .with_state(Arc::clone(&shared_stats))
        .layer(cors);

    // Optionally serve the pre-built React SPA from a runtime directory.
    let app: Router = if let Some(ref dir) = opt.static_dir {
        let index = dir.join("index.html");
        let serve_dir = ServeDir::new(dir).not_found_service(ServeFile::new(&index));
        info!(
            "Serving static files from '{}' with SPA fallback to '{}'",
            dir.display(),
            index.display()
        );
        // API routes are nested first so they shadow any static files at /api/*.
        api_router.fallback_service(serve_dir)
    } else {
        api_router
    };

    info!("HTTP server listening on {}", opt.listen);

    let listener = tokio::net::TcpListener::bind(opt.listen)
        .await
        .with_context(|| format!("Failed to bind to {}", opt.listen))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Axum server error")?;

    info!("Shutdown complete.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Graceful-shutdown signal handler
// ---------------------------------------------------------------------------

async fn shutdown_signal() {
    signal::ctrl_c()
        .await
        .expect("failed to install Ctrl-C handler");
    info!("Received Ctrl-C, shutting down.");
}

// ---------------------------------------------------------------------------
// BPF map reader
// ---------------------------------------------------------------------------

/// Read `PACKET_COUNTS` from the BPF map, aggregate per-CPU values, and
/// return a sorted Vec of `StatEntry`.
fn read_stats(bpf: &Ebpf) -> Vec<StatEntry> {
    let map = match bpf.map("PACKET_COUNTS") {
        Some(m) => m,
        None => {
            warn!("PACKET_COUNTS map not found");
            return Vec::new();
        }
    };

    let counts: PerCpuHashMap<_, PortKey, u64> = match PerCpuHashMap::try_from(map) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to open PACKET_COUNTS map: {}", e);
            return Vec::new();
        }
    };

    let mut entries: Vec<StatEntry> = counts
        .iter()
        .filter_map(|result| {
            let (key, per_cpu_values) = result.ok()?;
            let total: u64 = per_cpu_values.iter().sum();
            let protocol = match key.protocol {
                p if p == PROTO_TCP => "tcp".to_string(),
                p if p == PROTO_UDP => "udp".to_string(),
                other => format!("proto_{}", other),
            };
            Some(StatEntry {
                protocol,
                port: key.port,
                count: total,
            })
        })
        .collect();

    // Sort by count descending, then port ascending for stable output.
    entries.sort_unstable_by(|a, b| b.count.cmp(&a.count).then(a.port.cmp(&b.port)));
    entries
}

// ---------------------------------------------------------------------------
// HTTP handlers
// ---------------------------------------------------------------------------

/// `GET /api/health` — simple liveness probe.
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}

/// `GET /api/stats` — returns a JSON array of per-port packet counts.
///
/// Response shape (array):
/// ```json
/// [
///   { "protocol": "tcp", "port": 443, "count": 1024 },
///   { "protocol": "udp", "port": 53,  "count":  300 }
/// ]
/// ```
async fn stats_handler(State(stats): State<SharedStats>) -> impl IntoResponse {
    let snapshot = stats.read().await;
    (StatusCode::OK, Json(snapshot.clone()))
}
