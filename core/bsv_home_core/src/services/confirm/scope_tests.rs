use std::sync::Arc;

use futures::executor::block_on;
use primitives::{Account, Asset, AssetId, Chain, FeePriority, TransactionInputType, Wallet, known_assets::TRON_USDT};

use super::{BsvHomeConfirmFeeSelection, BsvHomeConfirmInput, BsvHomeConfirmLoadOptions, testkit::ConfirmTestkit};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};
use crate::testkit::TestAlienProvider;

#[test]
fn test_wowpay_confirm_rejects_hidden_assets_and_fee_choices_before_loading() {
    block_on(async {
        let wallet = Wallet::mock_with_accounts(vec![
            Account::mock(Chain::BitcoinSv, "sender"),
            Account::mock(Chain::Tron, "legacy-tron"),
            Account::mock(Chain::Ethereum, "legacy"),
        ]);
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let kit = ConfirmTestkit::with_environment(wallet.clone(), wallet.clone(), provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
        for asset in [
            Asset::from_chain(Chain::Ethereum),
            Asset::from_chain(Chain::Tron),
            TRON_USDT.clone(),
            Asset {
                id: AssetId::from_token(Chain::Tron, "counterfeit-usdt"),
                ..TRON_USDT.clone()
            },
        ] {
            let transfer = BsvHomeTransferData {
                input_type: TransactionInputType::Transfer { asset },
                recipient: BsvHomeRecipient::address("receiver".into()),
                value: 1.into(),
                use_max_amount: false,
            };
            assert!(kit.service.confirm_input(wallet.clone(), transfer).is_err());
        }
        for asset in [Asset::from_chain(Chain::BitcoinSv)] {
            assert!(kit.confirm.require_input_assets(&TransactionInputType::Transfer { asset }).is_ok());
        }
        let input = BsvHomeConfirmInput {
            from: wallet.account(Chain::BitcoinSv).unwrap().clone(),
            transfer: BsvHomeTransferData {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::BitcoinSv),
                },
                recipient: BsvHomeRecipient::address("receiver".into()),
                value: 1.into(),
                use_max_amount: false,
            },
        };
        assert!(
            kit.confirm
                .load(
                    wallet.id,
                    input,
                    BsvHomeConfirmLoadOptions {
                        fee_selection: BsvHomeConfirmFeeSelection::Priority { priority: FeePriority::Normal },
                        fee_asset_id: Some(Chain::Tron.as_asset_id())
                    }
                )
                .await
                .is_err()
        );
        assert!(provider.requested_paths().is_empty());
        assert!(kit.balances.requests.lock().unwrap().is_empty());
    });
}
