use std::collections::HashMap;

use async_trait::async_trait;
use primitives::perpetual::PerpetualData;
use primitives::{PerpetualMarketData, PerpetualPosition, PerpetualProvider, WalletId};

use super::BsvHomePerpetualStore;
use crate::services::error::BsvHomeServiceError;

pub struct UnusedPerpetualStore;

#[async_trait]
impl BsvHomePerpetualStore for UnusedPerpetualStore {
    async fn save_perpetuals(&self, _data: Vec<PerpetualData>) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not write perpetuals");
    }

    async fn set_pinned(&self, _perpetual_ids: Vec<String>, _pinned: bool) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not pin perpetuals");
    }

    async fn delete_perpetuals(&self) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must preserve perpetuals");
    }

    async fn get_positions(&self, _wallet_id: WalletId, _provider: PerpetualProvider) -> Result<Vec<PerpetualPosition>, BsvHomeServiceError> {
        panic!("selection capabilities must not read positions");
    }

    async fn get_position_ids(&self, _wallet_id: WalletId, _provider: PerpetualProvider) -> Result<Vec<String>, BsvHomeServiceError> {
        panic!("selection capabilities must not read position IDs");
    }

    async fn update_positions(&self, _wallet_id: WalletId, _positions: Vec<PerpetualPosition>, _delete_ids: Vec<String>) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must preserve positions");
    }

    async fn update_market(&self, _market: PerpetualMarketData) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not update markets");
    }

    async fn update_prices(&self, _prices: HashMap<String, f64>) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not update prices");
    }
}
