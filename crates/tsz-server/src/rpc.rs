use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct NodeRpc {
    endpoint: String,
    client: Client,
    request_id: std::sync::Arc<AtomicU64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain: String,
    pub blocks: u64,
    #[serde(default)]
    pub bestblockhash: String,
    #[serde(default)]
    pub verificationprogress: f64,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    result: Option<T>,
    error: Option<Value>,
}

impl NodeRpc {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: Client::new(),
            request_id: Default::default(),
        }
    }

    pub async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let response = self
            .client
            .post(&self.endpoint)
            .json(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .send()
            .await
            .with_context(|| format!("calling Zakura {method}"))?;
        let status = response.status();
        let envelope: Envelope<T> = response.json().await.context("decoding Zakura response")?;
        if let Some(error) = envelope.error {
            bail!("Zakura {method} failed: {error}");
        }
        if !status.is_success() {
            bail!("Zakura {method} returned HTTP {status}");
        }
        envelope
            .result
            .context("Zakura response did not contain a result")
    }

    pub async fn chain_info(&self) -> Result<ChainInfo> {
        self.call("getblockchaininfo", json!([])).await
    }
    pub async fn generate(&self, blocks: u32) -> Result<Vec<String>> {
        self.call("generate", json!([blocks])).await
    }
    pub async fn block(&self, id: &str) -> Result<Value> {
        self.call("getblock", json!([id, 2])).await
    }
    pub async fn transaction(&self, txid: &str) -> Result<Value> {
        self.call("getrawtransaction", json!([txid, 1])).await
    }
    pub async fn mempool(&self) -> Result<Vec<String>> {
        self.call("getrawmempool", json!([])).await
    }
}
