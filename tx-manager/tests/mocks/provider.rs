use async_trait::async_trait;
use ethers::providers::{FromErr, JsonRpcClient, Middleware, ProviderError};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{U256, U64};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug)]
pub struct Provider<M: Middleware> {
    inner: M,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderOutput<M: Middleware> {
    #[error("todo: {0}")]
    InnerError(M::Error),
}

impl<M: Middleware> FromErr<M::Error> for ProviderOutput<M> {
    fn from(src: M::Error) -> ProviderOutput<M> {
        ProviderOutput::InnerError(src)
    }
}

#[async_trait]
impl<M: Middleware> Middleware for Provider<M> {
    type Error = ProviderOutput<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        return &self.inner;
    }

    async fn get_block_number(&self) -> Result<U64, Self::Error> {
        todo!();
    }

    async fn estimate_gas(
        &self,
        tx: &TypedTransaction,
    ) -> Result<U256, Self::Error> {
        todo!();
    }
}

// Mock JSON RPC Client.

#[derive(Debug, thiserror::Error)]
enum JsonRpcClientError {}

impl From<JsonRpcClientError> for ProviderError {
    fn from(err: JsonRpcClientError) -> Self {
        ProviderError::JsonRpcClientError(Box::new(err))
    }
}

#[derive(Debug)]
struct MockJsonRpcClient {}

#[async_trait]
impl JsonRpcClient for MockJsonRpcClient {
    type Error = JsonRpcClientError;

    async fn request<T: Serialize + Send + Sync, R: DeserializeOwned>(
        &self,
        _: &str,
        _: T,
    ) -> Result<R, JsonRpcClientError> {
        unreachable!()
    }
}
