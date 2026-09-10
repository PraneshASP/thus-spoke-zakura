use std::{path::PathBuf, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::{
    db::{Account, Activity, Store, ZATOSHIS_PER_ZEC},
    rpc::{ChainInfo, NodeRpc},
    wallet::RealWallet,
};

#[derive(Clone)]
pub struct AppState(Arc<Inner>);
struct Inner {
    store: Store,
    wallet: RealWallet,
    rpc: NodeRpc,
    instance: String,
    events: broadcast::Sender<String>,
}

impl AppState {
    pub fn new(store: Store, wallet: RealWallet, rpc: String, instance: String) -> Self {
        let (events, _) = broadcast::channel(128);
        Self(Arc::new(Inner {
            store,
            wallet,
            rpc: NodeRpc::new(rpc),
            instance,
            events,
        }))
    }
}

pub fn router(state: AppState) -> Router {
    let static_dir = std::env::var("TSZ_WEB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("web/dist"));
    let index = static_dir.join("index.html");
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/status", get(status))
        .route("/api/v1/accounts", get(accounts))
        .route("/api/v1/activity", get(activity))
        .route("/api/v1/send", post(send))
        .route("/api/v1/faucet", post(faucet))
        .route("/api/v1/mine", post(mine))
        .route("/api/v1/dev/seed", post(seed))
        .route("/api/v1/blocks", get(blocks))
        .route("/api/v1/blocks/{id}", get(block))
        .route("/api/v1/transactions/{txid}", get(transaction))
        .route("/api/v1/mempool", get(mempool))
        .route("/api/v1/addresses/{address}", get(address))
        .route("/api/v1/search", get(search))
        .route("/api/v1/events", get(events))
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index)))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Response {
    let node = state.0.rpc.chain_info().await.ok();
    let wallet = state.0.wallet.sync().await;
    let status = if node.is_some() && wallet.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({"ok": node.is_some() && wallet.is_ok(), "instance": state.0.instance, "node": node, "wallet": wallet.err().map(|e| e.to_string())})),
    )
        .into_response()
}

#[derive(Serialize)]
struct Status {
    instance: String,
    node: Option<ChainInfo>,
    account_count: usize,
    auto_mine: bool,
    network: &'static str,
}
async fn status(State(state): State<AppState>) -> ApiResult<Json<Status>> {
    Ok(Json(Status {
        instance: state.0.instance.clone(),
        node: state.0.rpc.chain_info().await.ok(),
        account_count: state.0.store.accounts()?.len(),
        auto_mine: true,
        network: "Regtest",
    }))
}
async fn accounts(State(state): State<AppState>) -> ApiResult<Json<Vec<Account>>> {
    state.0.wallet.sync().await?;
    let mut accounts = state.0.store.accounts()?;
    state.0.wallet.apply_balances(&mut accounts).await?;
    Ok(Json(accounts))
}

#[derive(Deserialize)]
struct Page {
    limit: Option<u32>,
}
async fn activity(
    State(state): State<AppState>,
    Query(page): Query<Page>,
) -> ApiResult<Json<Vec<Activity>>> {
    Ok(Json(state.0.store.activities(page.limit.unwrap_or(30))?))
}

#[derive(Deserialize)]
struct SendRequest {
    from_account: u8,
    to_account: u8,
    source_pool: String,
    destination_pool: String,
    amount_zatoshi: u64,
    idempotency_key: String,
}
async fn send(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> ApiResult<Json<Activity>> {
    require_key(&req.idempotency_key)?;
    if let Some(existing) = state.0.store.activity_for_key(&req.idempotency_key)? {
        return Ok(Json(existing));
    }
    state.0.wallet.sync().await?;
    let destination = state.0.store.account(req.to_account)?;
    let address = if req.destination_pool == "transparent" {
        destination.transparent_address
    } else if req.destination_pool == "orchard" {
        destination.unified_address
    } else {
        return Err(ApiError::bad_request(
            "destination_pool must be transparent or orchard",
        ));
    };
    let txid = state
        .0
        .wallet
        .send(
            &state.0.store.seed()?,
            req.from_account,
            &req.source_pool,
            &address,
            req.amount_zatoshi,
        )
        .await?;
    let pending = state.0.store.transfer(
        req.from_account,
        req.to_account,
        &req.source_pool,
        &req.destination_pool,
        req.amount_zatoshi,
        &req.idempotency_key,
        &txid,
    )?;
    Ok(Json(confirm_after_mining(&state, pending).await?))
}

#[derive(Deserialize)]
struct FaucetRequest {
    account_id: u8,
    pool: String,
    amount_zatoshi: u64,
    idempotency_key: String,
}
async fn faucet(
    State(state): State<AppState>,
    Json(req): Json<FaucetRequest>,
) -> ApiResult<Json<Activity>> {
    require_key(&req.idempotency_key)?;
    if let Some(existing) = state.0.store.activity_for_key(&req.idempotency_key)? {
        return Ok(Json(existing));
    }
    if req.amount_zatoshi > 5 * ZATOSHIS_PER_ZEC {
        return Err(ApiError::bad_request(
            "a faucet request is limited to 5 ZEC",
        ));
    }
    let destination = state.0.store.account(req.account_id)?;
    let address = match req.pool.as_str() {
        "transparent" => destination.transparent_address,
        "orchard" => destination.unified_address,
        _ => return Err(ApiError::bad_request("pool must be transparent or orchard")),
    };
    let treasury = state.0.store.account(1)?;
    // The wallet starts scanning at block 2 because lightwalletd reserves height 0
    // as an unspecified BlockId. At height 102, block 2 is the first visible mature reward.
    let hashes = state.0.rpc.generate(102).await?;
    state.0.wallet.sync().await?;
    let mature_block = state.0.rpc.block(&hashes[1]).await?;
    let mature_tx = mature_block
        .pointer("/tx/0/hex")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Zakura block omitted coinbase transaction hex"))?;
    state.0.wallet.enhance_transaction(mature_tx, 2).await?;
    state
        .0
        .wallet
        .shield_coinbase(
            &state.0.store.seed()?,
            &treasury.transparent_address,
            &treasury.unified_address,
        )
        .await?;
    state.0.rpc.generate(1).await?;
    state.0.wallet.sync().await?;
    let txid = state
        .0
        .wallet
        .send(
            &state.0.store.seed()?,
            1,
            "orchard",
            &address,
            req.amount_zatoshi,
        )
        .await?;
    let pending = state.0.store.faucet(
        req.account_id,
        &req.pool,
        req.amount_zatoshi,
        &req.idempotency_key,
        &txid,
    )?;
    Ok(Json(confirm_after_mining(&state, pending).await?))
}

#[derive(Deserialize)]
struct MineRequest {
    blocks: u32,
}
async fn mine(
    State(state): State<AppState>,
    Json(req): Json<MineRequest>,
) -> ApiResult<Json<Value>> {
    if !(1..=10_000).contains(&req.blocks) {
        return Err(ApiError::bad_request("blocks must be between 1 and 10,000"));
    }
    let hashes = state.0.rpc.generate(req.blocks).await?;
    notify(&state, "chain");
    Ok(Json(json!({"blocks":hashes.len(),"hashes":hashes})))
}

#[derive(Deserialize)]
struct SeedRequest {
    confirmation: String,
}
async fn seed(
    State(state): State<AppState>,
    Json(req): Json<SeedRequest>,
) -> ApiResult<Json<Value>> {
    if req.confirmation != "I understand this seed is for regtest only" {
        return Err(ApiError::bad_request("exact confirmation phrase required"));
    }
    Ok(Json(
        json!({"seed_hex":state.0.store.seed()?,"warning":"Never send real funds to this development seed."}),
    ))
}
async fn block(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(state.0.rpc.block(&id).await?))
}
#[derive(Deserialize)]
struct BlocksQuery {
    limit: Option<u32>,
    before: Option<u64>,
}
async fn blocks(
    State(state): State<AppState>,
    Query(query): Query<BlocksQuery>,
) -> ApiResult<Json<Value>> {
    let info = state.0.rpc.chain_info().await?;
    let end = query.before.unwrap_or(info.blocks).min(info.blocks);
    let limit = query.limit.unwrap_or(20).clamp(1, 50) as u64;
    let start = end.saturating_sub(limit.saturating_sub(1));
    let mut page = Vec::new();
    for height in (start..=end).rev() {
        page.push(state.0.rpc.block(&height.to_string()).await?);
    }
    Ok(Json(
        json!({"blocks":page,"next_before":start.checked_sub(1)}),
    ))
}
async fn transaction(
    State(state): State<AppState>,
    Path(txid): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(state.0.rpc.transaction(&txid).await?))
}
async fn mempool(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"transactions":state.0.rpc.mempool().await?})))
}
async fn address(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> ApiResult<Json<Value>> {
    if !address.starts_with('t') {
        return Err(ApiError::bad_request(
            "only transparent addresses have public explorer activity",
        ));
    }
    let balance: Value = state
        .0
        .rpc
        .call("getaddressbalance", json!([{"addresses":[address]}]))
        .await?;
    Ok(Json(json!({"address":address,"balance":balance})))
}
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}
async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<Value>> {
    if query.q.starts_with('t') {
        return address(State(state), Path(query.q)).await;
    }
    if let Ok(block) = state.0.rpc.block(&query.q).await {
        return Ok(Json(json!({"type":"block","value":block})));
    }
    let tx = state.0.rpc.transaction(&query.q).await?;
    Ok(Json(json!({"type":"transaction","value":tx})))
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let mut receiver = state.0.events.subscribe();
    let stream = async_stream::stream! { loop { match receiver.recv().await { Ok(data) => yield Ok(Event::default().event("update").data(data)), Err(broadcast::error::RecvError::Lagged(_)) => continue, Err(_) => break } } };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn confirm_after_mining(state: &AppState, pending: Activity) -> ApiResult<Activity> {
    match state.0.rpc.generate(1).await {
        Ok(hashes) => {
            let confirmed = state.0.store.confirm(
                &pending.id,
                hashes.first().map(String::as_str).unwrap_or(""),
            )?;
            notify(state, "wallet");
            Ok(confirmed)
        }
        Err(error) => {
            tracing::warn!(%error, activity = %pending.id, "transaction recorded but auto-mine failed");
            Ok(pending)
        }
    }
}
fn notify(state: &AppState, topic: &str) {
    let _ = state.0.events.send(topic.to_owned());
}
fn require_key(key: &str) -> ApiResult<()> {
    if key.len() < 8 || key.len() > 128 {
        Err(ApiError::bad_request(
            "idempotency_key must contain 8-128 characters",
        ))
    } else {
        Ok(())
    }
}

type ApiResult<T> = Result<T, ApiError>;
struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"message":self.message,"status":self.status.as_u16()}})),
        )
            .into_response()
    }
}
