use std::{collections::HashMap, sync::Arc};

use bsv_home_client::ReqwestClient;
use bsv_home_ton::rpc::client::TonClient;

use crate::config::NFTProviderConfig;
use crate::provider::NFTProvider;
use crate::providers::ton::provider::TonNftProvider;
use crate::providers::{AlchemyClient, MagicEdenSolanaClient, OpenSeaClient};

pub struct NFTProviderFactory;

impl NFTProviderFactory {
    pub fn new_providers(config: NFTProviderConfig) -> Vec<Arc<dyn NFTProvider>> {
        let client = ReqwestClient::new(String::new(), bsv_home_client::reqwest_client());
        let alchemy_client = config.alchemy.configure_client(client.clone());
        let opensea_client = config
            .opensea
            .configure_client(client.clone())
            .with_default_headers(HashMap::from([("x-api-key".to_string(), config.opensea.key)]));
        let magiceden_client = config
            .magiceden
            .configure_client(client.clone())
            .with_default_headers(HashMap::from([("Authorization".to_string(), format!("Bearer {}", config.magiceden.key))]));
        let ton_client = config.ton.configure_client(client);

        vec![
            Arc::new(OpenSeaClient::new(opensea_client)),
            Arc::new(MagicEdenSolanaClient::new(magiceden_client)),
            Arc::new(AlchemyClient::new(alchemy_client)),
            Arc::new(TonNftProvider::new(TonClient::new(ton_client), config.offchain)),
        ]
    }
}
