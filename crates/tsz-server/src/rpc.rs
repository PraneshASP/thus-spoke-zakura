use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const CALL_TIMEOUT: Duration = Duration::from_secs(30);
const GENERATE_TIMEOUT: Duration = Duration::from_secs(3600);

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
        self.call_within(CALL_TIMEOUT, method, params).await
    }

    async fn call_within<T: DeserializeOwned>(
        &self,
        timeout: Duration,
        method: &str,
        params: Value,
    ) -> Result<T> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let response = self
            .client
            .post(&self.endpoint)
            .timeout(timeout)
            .json(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .send()
            .await
            .map_err(|error| {
                let context = if error.is_timeout() {
                    format!("Zakura {method} timed out after {timeout:?}")
                } else {
                    format!("calling Zakura {method}")
                };
                anyhow::Error::new(error).context(context)
            })?;
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
        self.call_within(GENERATE_TIMEOUT, "generate", json!([blocks]))
            .await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn call_gives_up_on_a_node_that_never_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rpc = NodeRpc::new(format!("http://{}", listener.local_addr().unwrap()));
        let err = rpc
            .call_within::<Value>(Duration::from_millis(50), "getblockcount", json!([]))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "Zakura getblockcount timed out after 50ms");
    }
}
