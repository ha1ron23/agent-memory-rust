mod memory;
mod wal;
mod layers;
mod embeddings;

use memory::{AgentMemory, MemoryEdge, MemoryNode};
use layers::{MemoryLayers, ShortTermEvent, ReasoningStep, ToolCall};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::signal;
use tracing::{info, warn};

struct AppState {
    graph: Arc<RwLock<AgentMemory>>,
    layers: Arc<RwLock<MemoryLayers>>,
}

#[derive(Debug, Deserialize)]
struct AddMemoryRequest {
    content: String,
    mem_type: String,
    id: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct AddRelationRequest {
    from: String,
    to: String,
    relation: String,
    weight: f32,
}

#[derive(Debug, Deserialize)]
struct QueryRequest {
    keyword: String,
}

#[derive(Debug, Deserialize)]
struct ContextRequest {
    max: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SemanticSearchRequest {
    q: String,
    top: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ShortTermRequest {
    role: String,
    content: String,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ReasoningRequest {
    step_type: String,
    content: String,
    parent_id: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    ok: bool,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

impl ApiResponse {
    fn success(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
    fn error(msg: String) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let mut graph = AgentMemory::new();
    if let Err(e) = wal::recover_graph(&mut graph) {
        eprintln!("Graph WAL recovery error: {}", e);
    }
    info!("Graph contains {} nodes", graph.len());

    let (stm_events, reasoning_steps) = match wal::recover_layers() {
        Ok((ev, st)) => (ev, st),
        Err(e) => {
            eprintln!("Layers WAL recovery error: {}", e);
            (vec![], vec![])
        }
    };
    let mut layers = MemoryLayers::new(100);
    layers.restore_short_term(stm_events);
    layers.restore_reasoning(reasoning_steps);

    let state = Arc::new(AppState {
        graph: Arc::new(RwLock::new(graph)),
        layers: Arc::new(RwLock::new(layers)),
    });

    let app = axum::Router::new()
        .route("/memory", post(add_memory))
        .route("/relation", post(add_relation))
        .route("/query", get(query))
        .route("/context", get(context))
        .route("/semantic_search", get(semantic_search))
        .route("/short_term", post(add_short_term))
        .route("/short_term/context", get(get_short_term_context))
        .route("/reasoning", post(add_reasoning_step))
        .route("/reasoning/trace", get(get_reasoning_trace))
        .with_state(state);

    let addr = "127.0.0.1:8080";
    info!("HTTP memory server listening on http://{}", addr);
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutting down gracefully...");
}

async fn add_memory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddMemoryRequest>,
) -> impl IntoResponse {
    let id = req.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let content = req.content.clone();

    let http_client = reqwest::Client::new();
    let embedding = match embeddings::fetch_embedding(&content, &http_client).await {
        Ok(vec) => Some(vec),
        Err(e) => {
            warn!("Embedding failed: {}", e);
            None
        }
    };

    let node = MemoryNode {
        id: id.clone(),
        content: req.content,
        mem_type: req.mem_type,
        created_at: now_secs(),
        metadata: req.metadata.unwrap_or_default(),
        embedding,
    };

    let cmd = wal::WalCommand::AddNode { node: node.clone() };
    if let Err(e) = wal::append_command(cmd).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(format!("WAL error: {}", e))),
        );
    }

    let mut graph = state.graph.write().await;
    match graph.add_node(node) {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::success(serde_json::json!({ "id": id }))),
        ),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ApiResponse::error(e))),
    }
}

async fn add_relation(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddRelationRequest>,
) -> impl IntoResponse {
    let edge = MemoryEdge {
        relation: req.relation,
        weight: req.weight,
        created_at: now_secs(),
    };
    let cmd = wal::WalCommand::AddEdge {
        from_id: req.from.clone(),
        to_id: req.to.clone(),
        edge: edge.clone(),
    };
    if let Err(e) = wal::append_command(cmd).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(format!("WAL error: {}", e))),
        );
    }
    let mut graph = state.graph.write().await;
    match graph.add_edge(&req.from, &req.to, edge) {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::success(serde_json::json!({}))),
        ),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ApiResponse::error(e))),
    }
}

async fn query(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<QueryRequest>,
) -> impl IntoResponse {
    let graph = state.graph.read().await;
    let results = graph
        .search_by_keyword(&params.keyword)
        .into_iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "content": n.content,
                "type": n.mem_type,
            })
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({ "results": results }))),
    )
}

async fn context(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ContextRequest>,
) -> impl IntoResponse {
    let graph = state.graph.read().await;
    let limit = params.max.unwrap_or(10);
    let nodes = graph.get_recent(limit);
    let list = nodes
        .into_iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "content": n.content,
                "type": n.mem_type,
            })
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({ "context": list }))),
    )
}

async fn semantic_search(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<SemanticSearchRequest>,
) -> impl IntoResponse {
    let http_client = reqwest::Client::new();
    let query_embed = match embeddings::fetch_embedding(&params.q, &http_client).await {
        Ok(vec) => vec,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error(format!("Embedding failed: {}", e))),
            );
        }
    };
    let graph = state.graph.read().await;
    let top = params.top.unwrap_or(5);
    let results = graph.semantic_search(&query_embed, top);
    let list = results
        .into_iter()
        .map(|(node, score)| {
            serde_json::json!({
                "id": node.id,
                "content": node.content,
                "type": node.mem_type,
                "score": score,
            })
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({ "results": list }))),
    )
}

async fn add_short_term(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ShortTermRequest>,
) -> impl IntoResponse {
    let event = ShortTermEvent::new(
        req.role,
        req.content,
        req.metadata.unwrap_or(serde_json::json!({})),
    );
    let cmd = wal::WalCommand::AddShortTermEvent {
        event: event.clone(),
    };
    if let Err(e) = wal::append_command(cmd).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(format!("WAL error: {}", e))),
        );
    }
    let mut layers = state.layers.write().await;
    layers.add_short_term_event(event);
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({}))),
    )
}

async fn get_short_term_context(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ContextRequest>,
) -> impl IntoResponse {
    let layers = state.layers.read().await;
    let n = params.max.unwrap_or(10);
    let events = layers.get_short_term_context(n);
    let list = events
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "role": e.role,
                "content": e.content,
                "timestamp": e.timestamp,
                "metadata": e.metadata,
            })
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({ "context": list }))),
    )
}

async fn add_reasoning_step(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReasoningRequest>,
) -> impl IntoResponse {
    let step = ReasoningStep::new(
        req.step_type,
        req.content,
        req.parent_id,
        req.tool_calls,
    );
    let cmd = wal::WalCommand::AddReasoningStep { step: step.clone() };
    if let Err(e) = wal::append_command(cmd).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(format!("WAL error: {}", e))),
        );
    }
    let mut layers = state.layers.write().await;
    layers.add_reasoning_step(step);
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({}))),
    )
}

async fn get_reasoning_trace(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ContextRequest>,
) -> impl IntoResponse {
    let layers = state.layers.read().await;
    let limit = params.max.unwrap_or(20);
    let trace = layers.get_reasoning_trace(limit);
    let list = trace
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "type": s.step_type,
                "content": s.content,
                "timestamp": s.timestamp,
                "parent_id": s.parent_id,
                "tool_calls": s.tool_calls,
            })
        })
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(ApiResponse::success(serde_json::json!({ "trace": list }))),
    )
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
