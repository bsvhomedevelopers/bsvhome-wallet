#![allow(clippy::result_large_err)]

pub mod model;
pub mod rules;

use std::sync::Arc;

use primitives::{Asset, Chain, Currency, PerpetualDirection, StakeType};

pub use model::{
    BsvHomeAmountEarnType, BsvHomeAmountEntry, BsvHomeAmountEquivalent, BsvHomeAmountError, BsvHomeAmountInput, BsvHomeAmountInputType, BsvHomeAmountMaxEntry,
    BsvHomeAmountPerpetualPosition, BsvHomeAmountStakeType, BsvHomeAmountTransfer, BsvHomeAmountType, BsvHomePerpetualAutoclose,
};

use crate::config::perpetual_config::{leverage_options, select_leverage};

use crate::models::BsvHomeEarnType;
use crate::models::custom_types::BsvHomeBigInt;
use crate::services::error::BsvHomeServiceError;
use crate::services::perpetual::BsvHomePerpetualPositionAction;
use crate::services::perpetual::rules as perpetual_rules;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::stake::rules as stake_rules;
use crate::services::stake::{BsvHomeStakeAmountInput, BsvHomeStakeService, BsvHomeStakeValidatorSelection};
use crate::services::transfer::rules as transfer_rules;
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeAmountService {
    stake: Arc<BsvHomeStakeService>,
    preferences: Arc<BsvHomePreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeAmountService {
    #[uniffi::constructor]
    pub fn new(stake: Arc<BsvHomeStakeService>, preferences: Arc<BsvHomePreferencesService>, session: Arc<BsvHomeWalletSessionService>) -> Self {
        Self { stake, preferences, session }
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn perpetual_leverage(&self, max_leverage: u8) -> u8 {
        select_leverage(self.preferences.get_perpetual_leverage(), &leverage_options(max_leverage))
    }

    pub fn perpetual_autoclose(&self, price: f64, direction: PerpetualDirection, leverage: u8) -> BsvHomePerpetualAutoclose {
        rules::perpetual_autoclose(
            price,
            direction,
            leverage,
            self.preferences.get_perpetual_take_profit_percent(),
            self.preferences.get_perpetual_stop_loss_percent(),
        )
    }

    pub fn perpetual_transfer_data(
        &self,
        action: BsvHomePerpetualPositionAction,
        value: BsvHomeBigInt,
        use_max_amount: bool,
        leverage: u8,
        take_profit: Option<f64>,
        stop_loss: Option<f64>,
    ) -> BsvHomeTransferData {
        perpetual_rules::order_transfer(action, value, use_max_amount, leverage, take_profit, stop_loss)
    }

    pub fn perpetual_amount_type(&self, action: BsvHomePerpetualPositionAction, leverage: u8) -> BsvHomeAmountType {
        rules::perpetual_amount_type(&action, leverage)
    }

    pub fn stake_validator_selection(&self, chain: Chain, input: BsvHomeStakeAmountInput) -> BsvHomeStakeValidatorSelection {
        stake_rules::validator_selection(chain, &input)
    }

    pub fn earn_amount_type(&self, earn_type: BsvHomeEarnType) -> BsvHomeAmountType {
        rules::earn_amount_type(earn_type)
    }

    pub async fn transfer_data(
        &self,
        asset: Asset,
        transfer: BsvHomeAmountTransfer,
        value: BsvHomeBigInt,
        use_max_amount: bool,
    ) -> Result<BsvHomeTransferData, BsvHomeServiceError> {
        self.require_asset(&asset.id)?;
        let owner = match transfer {
            BsvHomeAmountTransfer::Withdraw => {
                let wallet = self.session.current_wallet().await?;
                let account = wallet.account(asset.chain()).ok_or_else(|| BsvHomeServiceError::NotFound {
                    msg: format!("wallet {} has no {} account", wallet.id.id(), asset.chain()),
                })?;
                Some(BsvHomeRecipient::named(account.address.clone(), wallet.name.clone()))
            }
            BsvHomeAmountTransfer::Send { .. } | BsvHomeAmountTransfer::Deposit => None,
        };
        rules::transfer_data(asset, transfer, owner, value, use_max_amount)
    }

    pub fn stake_transfer_data(&self, asset: Asset, stake_type: StakeType, value: BsvHomeBigInt, use_max_amount: bool) -> BsvHomeTransferData {
        transfer_rules::stake_transfer_data(asset, stake_type, value, use_max_amount)
    }

    pub async fn earn_transfer_data(
        &self,
        asset: Asset,
        earn_type: BsvHomeEarnType,
        value: BsvHomeBigInt,
        use_max_amount: bool,
    ) -> Result<BsvHomeTransferData, BsvHomeServiceError> {
        self.require_asset(&asset.id)?;
        let wallet = self.session.current_wallet().await?;
        let account = wallet.account(asset.chain()).ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} has no {} account", wallet.id.id(), asset.chain()),
        })?;
        let data = self
            .stake
            .get_earn_data(asset.id.clone(), account.address.clone(), value.to_string(), earn_type.clone())
            .await?;
        Ok(transfer_rules::earn_transfer_data(asset, earn_type, data, value, use_max_amount))
    }
}

impl BsvHomeAmountService {
    pub(crate) fn require_asset(&self, asset_id: &primitives::AssetId) -> Result<(), BsvHomeServiceError> {
        self.session.environment.require_asset(asset_id)
    }
}
