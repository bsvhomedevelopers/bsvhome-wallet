use std::future::Future;
use std::time::Instant;

use bitcoin::{
    Transaction,
    consensus::{deserialize, serialize},
};
use bsv_home_client::{Client, Target};

use super::{BsvClient, ProviderError, client::rate_limited, target::BsvTarget};
use crate::cold_wallet::MAX_PAYLOAD;

impl<C: Client> BsvClient<C> {
    pub async fn broadcast_cold_transaction(&self, raw: &str) -> Result<String, ProviderError> {
        self.broadcast_cold_transaction_checked(raw, &|| Ok(()), || async { Ok(()) }, &|| Ok(())).await
    }

    pub async fn broadcast_cold_transaction_checked<F, Fut>(
        &self,
        raw: &str,
        check: &(dyn Fn() -> Result<(), ProviderError> + Sync),
        before_send: F,
        before_submission: &(dyn Fn() -> Result<(), ProviderError> + Sync),
    ) -> Result<String, ProviderError>
    where
        F: Fn() -> Fut + Sync,
        Fut: Future<Output = Result<(), ProviderError>> + Send,
    {
        if raw.len() > MAX_PAYLOAD {
            return Err("Cold wallet signed transaction exceeds limit".into());
        }
        let bytes = hex::decode(raw)?;
        let transaction: Transaction = deserialize(&bytes)?;
        if serialize(&transaction) != bytes || transaction.input.iter().any(|input| !input.witness.is_empty()) {
            return Err("Cold wallet signed transaction serialization mismatch".into());
        }
        let expected = transaction.compute_txid().to_string();
        check()?;
        self.scheduler.acquire().await;
        check()?;
        before_send().await?;
        check()?;
        before_submission()?;
        let started = Instant::now();
        let response = self.client.post_text(&BsvTarget::Broadcast.path(), &serde_json::json!({"txhex":raw})).await;
        bsv_home_tracing::info_with_fields!(
            "BSV provider request",
            operation = "cold_broadcast",
            elapsed_ms = started.elapsed().as_millis(),
            success = response.is_ok(),
            limited = rate_limited(&response)
        );
        match response {
            Ok(returned) if returned.trim() == expected => Ok(expected),
            Ok(_) => Err("Cold wallet broadcast returned a different transaction ID".into()),
            Err(error) => match self.raw_transaction_checked(&expected, check).await {
                Ok(accepted) if accepted == hex::encode(bytes) => Ok(expected),
                Ok(_) => Err("Cold wallet accepted transaction differs from signed data".into()),
                Err(_) => Err(error.into()),
            },
        }
    }
}
