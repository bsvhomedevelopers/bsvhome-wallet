use bsv_home_client::{Target, build_path_with_query};

#[derive(Clone, Copy)]
pub enum Confirmation {
    Confirmed,
    Unconfirmed,
}
impl Confirmation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Unconfirmed => "unconfirmed",
        }
    }
}
#[derive(Clone)]
pub enum BsvTarget {
    ChainInfo,
    Block {
        height: u64,
    },
    FeeRecommendation,
    Transaction {
        txid: String,
    },
    RawTransaction {
        txid: String,
    },
    OutpointSpend {
        txid: String,
        vout: u32,
    },
    ScriptUnspent {
        script_hash: String,
        token: Option<String>,
    },
    ScriptHistory {
        script_hash: String,
        confirmation: Confirmation,
        token: Option<String>,
    },
    Unspent {
        address: String,
        token: Option<String>,
    },
    HistoryAll,
    History {
        address: String,
        confirmation: Confirmation,
        token: Option<String>,
    },
    Broadcast,
}
impl Target for BsvTarget {
    fn path(&self) -> String {
        match self {
            Self::ChainInfo => "/chain/info".to_string(),
            Self::Block { height } => format!("/block/height/{height}"),
            Self::FeeRecommendation => "/feerecommendation".to_string(),
            Self::Transaction { txid } => format!("/tx/hash/{txid}"),
            Self::RawTransaction { txid } => format!("/tx/{txid}/hex"),
            Self::OutpointSpend { txid, vout } => format!("/tx/{txid}/{vout}/spent"),
            Self::ScriptUnspent { script_hash, token } => {
                let path = format!("/script/{script_hash}/unspent/all");
                let parameters = token.iter().map(|token| ("token", token.clone())).collect::<Vec<_>>();
                build_path_with_query(&path, &parameters)
            }
            Self::ScriptHistory { script_hash, confirmation, token } => {
                let path = format!("/script/{script_hash}/{}/history", confirmation.as_str());
                let mut parameters = vec![("limit", "1000".to_string())];
                if let Some(token) = token {
                    parameters.push(("token", token.clone()));
                }
                build_path_with_query(&path, &parameters)
            }
            Self::Broadcast => "/tx/raw".to_string(),
            Self::HistoryAll => "/addresses/history/all".to_string(),
            Self::Unspent { address, token } => {
                let path = format!("/address/{address}/unspent/all");
                let parameters = token.iter().map(|token| ("token", token.clone())).collect::<Vec<_>>();
                build_path_with_query(&path, &parameters)
            }
            Self::History { address, confirmation, token } => {
                let path = format!("/address/{address}/{}/history", confirmation.as_str());
                let mut parameters = vec![("limit", "1000".to_string())];
                if let Some(token) = token {
                    parameters.push(("token", token.clone()));
                }
                build_path_with_query(&path, &parameters)
            }
        }
    }
}
