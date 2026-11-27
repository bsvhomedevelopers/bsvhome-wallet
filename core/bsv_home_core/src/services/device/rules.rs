use primitives::{Device, Wallet};

use crate::services::subscription::rules::wallet_subscriptions;

pub fn subscriptions_signature(wallets: &[Wallet]) -> String {
    let mut entries = Vec::new();
    for wallet in wallet_subscriptions(wallets) {
        for subscription in wallet.subscriptions {
            for chain in subscription.chains {
                entries.push(format!("{}/{}/{}", wallet.wallet_id.id(), chain.as_ref(), subscription.address));
            }
        }
    }
    entries.sort();
    entries.join(";")
}

pub fn device_changed(current: &Device, other: &Device) -> bool {
    current.id != other.id
        || current.token != other.token
        || current.locale != other.locale
        || current.version != other.version
        || current.currency != other.currency
        || current.is_push_enabled != other.is_push_enabled
        || current.is_price_alerts_enabled != other.is_price_alerts_enabled
        || current.subscriptions_version != other.subscriptions_version
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::currency::Currency;
    use primitives::{Account, Chain, WalletId, WalletSource};

    fn wallet(id: &str, accounts: Vec<Account>) -> Wallet {
        Wallet {
            id: WalletId::Multicoin(id.into()),
            name: "Test Wallet".into(),
            source: WalletSource::Create,
            ..Wallet::mock_with_accounts(accounts)
        }
    }

    #[test]
    fn test_signature_ignores_rename_and_pin_but_tracks_accounts_and_order() {
        let base = wallet("wallet1", vec![Account::mock(Chain::Ethereum, "0xabc")]);
        let renamed = Wallet {
            name: "Renamed".into(),
            is_pinned: true,
            ..base.clone()
        };
        let extended = Wallet {
            accounts: vec![Account::mock(Chain::Ethereum, "0xabc"), Account::mock(Chain::Bitcoin, "bc1xyz")],
            ..base.clone()
        };
        let other = wallet("wallet2", vec![Account::mock(Chain::Solana, "solana123")]);

        assert_eq!(subscriptions_signature(std::slice::from_ref(&base)), subscriptions_signature(&[renamed]));
        assert_ne!(subscriptions_signature(std::slice::from_ref(&base)), subscriptions_signature(&[extended]));
        assert_eq!(subscriptions_signature(&[base.clone(), other.clone()]), subscriptions_signature(&[other, base]));
    }

    #[test]
    fn test_subscriptions_signature_ignores_unsupported_backend_accounts_and_wallets() {
        let mainnet = wallet("wallet1", vec![Account::mock(Chain::Tron, "Tshared")]);
        let mixed = Wallet {
            accounts: vec![
                Account::mock(Chain::Tron, "Tshared"),
                Account::mock(Chain::TronShasta, "Tshared"),
                Account::mock(Chain::BitcoinSv, "bsv"),
            ],
            ..mainnet.clone()
        };
        let testnet = wallet("wallet2", vec![Account::mock(Chain::TronShasta, "Tother")]);
        let bsv = wallet("wallet3", vec![Account::mock(Chain::BitcoinSv, "bsv-only")]);
        assert_eq!(subscriptions_signature(&[mixed, testnet, bsv]), "multicoin_wallet1/tron/Tshared");
        assert_eq!(subscriptions_signature(&[mainnet]), "multicoin_wallet1/tron/Tshared");
    }

    #[test]
    fn test_subscriptions_signature_uses_grouped_effective_subscription_entries() {
        let grouped = wallet("wallet1", vec![Account::mock(Chain::Ethereum, "0xshared"), Account::mock(Chain::Polygon, "0xshared")]);
        let duplicated = Wallet {
            accounts: vec![
                Account::mock(Chain::Polygon, "0xshared"),
                Account::mock(Chain::Ethereum, "0xshared"),
                Account::mock(Chain::Ethereum, "0xshared"),
            ],
            ..grouped.clone()
        };
        let expected = "multicoin_wallet1/ethereum/0xshared;multicoin_wallet1/polygon/0xshared";
        assert_eq!(subscriptions_signature(&[grouped]), expected);
        assert_eq!(subscriptions_signature(&[duplicated]), expected);
    }

    #[test]
    fn test_device_changed_tracks_synced_fields_only() {
        let remote = Device::mock();

        assert!(!device_changed(&remote, &remote.clone()));
        assert!(device_changed(
            &remote,
            &Device {
                currency: Currency::EUR,
                ..remote.clone()
            }
        ));
        assert!(device_changed(
            &remote,
            &Device {
                subscriptions_version: 2,
                ..remote.clone()
            }
        ));
        assert!(!device_changed(
            &remote,
            &Device {
                model: "Other".into(),
                ..remote.clone()
            }
        ));
    }
}
