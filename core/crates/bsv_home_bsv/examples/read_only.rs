use std::error::Error;

use bsv_home_bsv::rpc::BsvClient;
use bsv_home_client::ReqwestClient;
use chain_traits::{ChainBalances, ChainState, ChainTransactionLoad, ChainTransactions, TransactionsRequest, TransactionsResult};
use primitives::{Asset, Chain, TransactionInputType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let url = primitives::node_config::default_node_url(Chain::BitcoinSv).ok_or("BSV default node missing")?;
    let client = BsvClient::new(ReqwestClient::new(url.to_string(), bsv_home_client::reqwest_client()));
    let chain_id = client.get_chain_id().await?;
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let balance = client.get_balance_coin(address.to_string()).await?;
    let history = client.get_transactions_by_address(TransactionsRequest::new(address.to_string(), 1000)).await?;
    let history_count = match history {
        TransactionsResult::Transactions(transactions) => transactions.len(),
        _ => return Err("BSV history response type mismatch".into()),
    };
    let rates = client
        .get_transaction_fee_rates(TransactionInputType::Transfer {
            asset: Asset::from_chain(Chain::BitcoinSv),
        })
        .await?;
    println!(
        "BSV mainnet identity={chain_id:?}, available_satoshis={}, pending_satoshis={}, history_count={history_count}, fee_sat_per_kb={}",
        balance.balance.available,
        balance.balance.pending_unconfirmed,
        rates.first().ok_or("fee quote missing")?.gas_price_type.gas_price()
    );
    Ok(())
}
