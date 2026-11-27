use crate::block_explorer::BlockExplorer;
use crate::explorers::metadata::{ADDRESS_PATH, Explorer, Metadata, TRANSACTION_PATH};

pub struct TronScan;

impl TronScan {
    pub fn boxed() -> Box<dyn BlockExplorer> {
        Self::with_base_url("https://tronscan.org/#")
    }

    pub fn shasta() -> Box<dyn BlockExplorer> {
        Self::with_base_url("https://shasta.tronscan.org/#")
    }

    fn with_base_url(base_url: &'static str) -> Box<dyn BlockExplorer> {
        Explorer::boxed(Metadata {
            name: "TRONSCAN",
            base_url,
            tx_path: TRANSACTION_PATH,
            address_path: ADDRESS_PATH,
            token_path: Some("/token20"),
            nft_path: None,
            validator_path: Some(ADDRESS_PATH),
        })
    }
}
