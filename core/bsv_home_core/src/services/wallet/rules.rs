use bsv_home_keystore::Mnemonic;
use primitives::{Account, AddressName, AddressType, Chain, NameRecord, VerificationStatus, Wallet, WalletId, WalletSource, WalletType};

use super::error::BsvHomeWalletImportError;
use super::model::{
    BsvHomeBsvHdImportMode, BsvHomeWalletImportKind, BsvHomeWalletImportType, BsvHomeWalletPlaceholder, BsvHomeWalletRow, BsvHomeWalletSecretKind, BsvHomeWalletSubtitle,
};
use crate::address::{checksum_address, validate_address};
use crate::keystore::BsvHomeKeystoreAccount;
use crate::signer::decode_private_key;

impl BsvHomeWalletImportType {
    pub fn validated(self) -> Result<Self, BsvHomeWalletImportError> {
        match self {
            Self::MulticoinPhrase { words, chains } => Ok(Self::MulticoinPhrase {
                words: validated_words(words)?,
                chains,
            }),
            Self::SinglePhrase { words, chain } => Ok(Self::SinglePhrase {
                words: validated_words(words)?,
                chain,
            }),
            Self::PrivateKey { value, chain } => {
                let value = value.trim().to_string();
                decode_private_key(chain, value.clone()).map_err(|_| BsvHomeWalletImportError::InvalidPrivateKey)?;
                Ok(Self::PrivateKey { value, chain })
            }
            Self::Address { address, chain } => {
                let address = checksum_address(address.trim(), chain);
                if !validate_address(&address, chain) {
                    return Err(BsvHomeWalletImportError::InvalidAddress);
                }
                Ok(Self::Address { address, chain })
            }
        }
    }
}

pub fn import_kinds(chain: Option<Chain>) -> Vec<BsvHomeWalletImportKind> {
    match chain {
        None => vec![BsvHomeWalletImportKind::Phrase],
        Some(chain) => [
            Some(BsvHomeWalletImportKind::Phrase),
            signer::supports_private_key_import(&chain).then_some(BsvHomeWalletImportKind::PrivateKey),
            Some(BsvHomeWalletImportKind::Address),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

pub fn bsv_hd_import_mode(kind: BsvHomeWalletImportKind, chain: Option<Chain>, input: &str, has_overrides: bool) -> Option<BsvHomeBsvHdImportMode> {
    let recovery = input.trim_start().starts_with('{') || input.contains(':');
    match kind {
        BsvHomeWalletImportKind::Phrase => match chain {
            Some(Chain::BitcoinSv) => Some(BsvHomeBsvHdImportMode::Recovery),
            None => Some(if recovery || has_overrides {
                BsvHomeBsvHdImportMode::Recovery
            } else {
                BsvHomeBsvHdImportMode::Multicoin
            }),
            Some(_) => None,
        },
        BsvHomeWalletImportKind::Address => {
            if chain == Some(Chain::BitcoinSv) && (input.trim_start().starts_with("xpub") || recovery) {
                Some(BsvHomeBsvHdImportMode::Observer)
            } else {
                None
            }
        }
        BsvHomeWalletImportKind::PrivateKey => None,
    }
}

pub fn import_request(
    kind: BsvHomeWalletImportKind,
    chain: Option<Chain>,
    input: &str,
    name_record: Option<&NameRecord>,
) -> Result<BsvHomeWalletImportType, BsvHomeWalletImportError> {
    let words = || input.split_whitespace().map(str::to_string).collect();
    match (kind, chain) {
        (BsvHomeWalletImportKind::Phrase, None) => Ok(BsvHomeWalletImportType::MulticoinPhrase {
            words: words(),
            chains: Chain::all(),
        }),
        (BsvHomeWalletImportKind::Phrase, Some(chain)) => Ok(BsvHomeWalletImportType::SinglePhrase { words: words(), chain }),
        (BsvHomeWalletImportKind::PrivateKey, Some(chain)) => Ok(BsvHomeWalletImportType::PrivateKey {
            value: input.trim().to_string(),
            chain,
        }),
        (BsvHomeWalletImportKind::Address, Some(chain)) => Ok(BsvHomeWalletImportType::Address {
            address: name_record
                .map(|record| record.address.trim())
                .filter(|address| !address.is_empty())
                .unwrap_or(input.trim())
                .to_string(),
            chain,
        }),
        (BsvHomeWalletImportKind::PrivateKey | BsvHomeWalletImportKind::Address, None) => Err(BsvHomeWalletImportError::MissingChain),
    }
}

pub fn import_name(name_record: Option<&NameRecord>, default_name: String) -> String {
    name_record
        .map(|record| record.name.trim())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or(default_name)
}

fn validated_words(words: Vec<String>) -> Result<Vec<String>, BsvHomeWalletImportError> {
    let words: Vec<String> = words.iter().flat_map(|word| word.split_whitespace()).map(|word| word.to_lowercase()).collect();
    let phrase = words.join(" ");
    let invalid = Mnemonic::invalid_words(&phrase);
    if !invalid.is_empty() {
        return Err(BsvHomeWalletImportError::InvalidSecretPhraseWords { words: invalid });
    }
    if !Mnemonic::is_valid(&phrase) {
        return Err(BsvHomeWalletImportError::InvalidSecretPhrase);
    }
    Ok(words)
}

#[derive(Debug, Clone, PartialEq)]
pub enum SecretExport {
    Words,
    PrivateKey(Chain),
    None,
}

pub fn secret_export(wallet: &Wallet) -> SecretExport {
    match wallet.wallet_type {
        WalletType::Multicoin | WalletType::Single => SecretExport::Words,
        WalletType::PrivateKey => wallet.accounts.first().map(|account| SecretExport::PrivateKey(account.chain)).unwrap_or(SecretExport::None),
        WalletType::View => SecretExport::None,
    }
}

pub fn secret_kind(wallet: &Wallet) -> Option<BsvHomeWalletSecretKind> {
    match secret_export(wallet) {
        SecretExport::Words => Some(BsvHomeWalletSecretKind::Phrase),
        SecretExport::PrivateKey(_) => Some(BsvHomeWalletSecretKind::PrivateKey),
        SecretExport::None => None,
    }
}

pub fn row(wallet: &Wallet) -> BsvHomeWalletRow {
    match &wallet.id {
        WalletId::Multicoin(_) => BsvHomeWalletRow {
            subtitle: BsvHomeWalletSubtitle::Multicoin,
            placeholder: BsvHomeWalletPlaceholder::Multicoin,
            shows_watch_badge: false,
        },
        WalletId::Single(chain, address) | WalletId::PrivateKey(chain, address) | WalletId::View(chain, address) => BsvHomeWalletRow {
            subtitle: BsvHomeWalletSubtitle::Account {
                chain: *chain,
                address: address.clone(),
            },
            placeholder: BsvHomeWalletPlaceholder::Chain { chain: *chain },
            shows_watch_badge: matches!(wallet.id, WalletId::View(..)),
        },
    }
}

pub fn view_wallet(name: String, chain: Chain, address: String) -> Wallet {
    Wallet {
        id: WalletId::View(chain, address.clone()),
        external_id: None,
        name,
        index: 0,
        wallet_type: WalletType::View,
        accounts: vec![Account {
            chain,
            address,
            derivation_path: String::new(),
            extended_public_key: Some(String::new()),
        }],
        is_pinned: false,
        image_url: None,
        source: WalletSource::Import,
    }
}

pub fn account(account: BsvHomeKeystoreAccount) -> Account {
    Account {
        chain: account.chain,
        address: account.address,
        derivation_path: account.derivation_path,
        extended_public_key: Some(account.public_key.unwrap_or_default()),
    }
}

pub fn next_wallet_index(wallets: &[Wallet]) -> i32 {
    wallets.iter().map(|wallet| wallet.index).max().map(|index| index + 1).unwrap_or(1)
}

fn missing_chains(wallet: &Wallet, chains: &[Chain]) -> Vec<Chain> {
    chains.iter().copied().filter(|chain| wallet.account(*chain).is_none()).collect()
}

pub fn wallets_missing_chains(wallets: Vec<Wallet>, chains: &[Chain]) -> Vec<(Wallet, Vec<Chain>)> {
    wallets
        .into_iter()
        .filter(|wallet| wallet.wallet_type == WalletType::Multicoin || wallet.wallet_type == WalletType::Single)
        .filter_map(|wallet| {
            let missing = missing_chains(&wallet, chains)
                .into_iter()
                .filter(|chain| wallet.wallet_type == WalletType::Multicoin || chain.is_testnet())
                .collect::<Vec<_>>();
            (!missing.is_empty()).then_some((wallet, missing))
        })
        .collect()
}

pub fn sorted_wallets(wallets: Vec<Wallet>) -> Vec<Wallet> {
    let mut sorted = wallets;
    sorted.sort_by_key(|wallet| (wallet.wallet_type.rank(), wallet.index));
    sorted
}

pub fn show_collections(wallet_type: WalletType, chains: &[Chain]) -> bool {
    match wallet_type {
        WalletType::Multicoin => true,
        WalletType::Single | WalletType::PrivateKey | WalletType::View => chains.first().is_some_and(|chain| chain.is_nft_supported()),
    }
}

pub fn wallet_address_names(wallet: &Wallet) -> Vec<AddressName> {
    wallet
        .accounts
        .iter()
        .map(|account| AddressName {
            chain: account.chain,
            address: account.address.clone(),
            name: wallet.name.clone(),
            address_type: AddressType::InternalWallet,
            status: VerificationStatus::Verified,
            image_url: None,
        })
        .collect()
}

pub fn next_current_wallet(wallets: &[Wallet]) -> Option<WalletId> {
    wallets
        .iter()
        .min_by_key(|wallet| (wallet.wallet_type.rank(), wallet.index))
        .map(|wallet| wallet.id.clone())
}

pub fn legacy_keystore_id(wallet: &Wallet) -> String {
    wallet.external_id.clone().unwrap_or_else(|| wallet.id.id())
}

pub fn existing_wallet(wallets: &[Wallet], wallet_id: &WalletId, wallet_type: WalletType) -> Option<Wallet> {
    wallets.iter().find(|wallet| wallet.id == *wallet_id && wallet.wallet_type == wallet_type).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::NameProvider;

    fn record(name: &str, address: &str) -> NameRecord {
        NameRecord {
            name: name.to_string(),
            chain: Chain::Ethereum,
            address: address.to_string(),
            provider: NameProvider::Ens,
        }
    }

    #[test]
    fn test_import_kinds_offer_a_private_key_only_where_the_chain_supports_it() {
        assert_eq!(import_kinds(None), vec![BsvHomeWalletImportKind::Phrase]);
        assert_eq!(
            import_kinds(Some(Chain::Ethereum)),
            vec![BsvHomeWalletImportKind::Phrase, BsvHomeWalletImportKind::PrivateKey, BsvHomeWalletImportKind::Address]
        );
        assert_eq!(import_kinds(Some(Chain::Bitcoin)), vec![BsvHomeWalletImportKind::Phrase, BsvHomeWalletImportKind::Address]);
        assert!(BsvHomeWalletImportKind::Phrase.protects_input() && BsvHomeWalletImportKind::PrivateKey.protects_input());
        assert!(!BsvHomeWalletImportKind::Address.protects_input());
        assert!(BsvHomeWalletImportKind::Phrase.supports_phrase_suggestions() && !BsvHomeWalletImportKind::Address.supports_phrase_suggestions());
        assert!(BsvHomeWalletImportKind::Address.shows_view_only_warning() && !BsvHomeWalletImportKind::Phrase.shows_view_only_warning());
    }

    #[test]
    fn test_import_request_splits_words_and_prefers_the_resolved_address() {
        let phrase = " alpha  bravo\tcharlie ";
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::Phrase, None, phrase, None),
            Ok(BsvHomeWalletImportType::MulticoinPhrase { words, chains }) if words == ["alpha", "bravo", "charlie"] && chains == Chain::all()
        ));
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::Phrase, Some(Chain::Solana), phrase, None),
            Ok(BsvHomeWalletImportType::SinglePhrase { words, chain: Chain::Solana }) if words.len() == 3
        ));
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::PrivateKey, Some(Chain::Ethereum), " 0xabc ", None),
            Ok(BsvHomeWalletImportType::PrivateKey { value, chain: Chain::Ethereum }) if value == "0xabc"
        ));
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::Address, Some(Chain::Ethereum), "vitalik.eth", Some(&record("vitalik.eth", "0xd8dA"))),
            Ok(BsvHomeWalletImportType::Address { address, chain: Chain::Ethereum }) if address == "0xd8dA"
        ));
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::Address, Some(Chain::Ethereum), " 0x123 ", Some(&record("", ""))),
            Ok(BsvHomeWalletImportType::Address { address, .. }) if address == "0x123"
        ));
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::Address, None, "0x123", None),
            Err(BsvHomeWalletImportError::MissingChain)
        ));
        assert!(matches!(
            import_request(BsvHomeWalletImportKind::PrivateKey, None, "0x123", None),
            Err(BsvHomeWalletImportError::MissingChain)
        ));
    }

    #[test]
    fn test_import_name_uses_the_resolved_name_unless_it_is_blank() {
        assert_eq!(import_name(Some(&record("vitalik.eth", "0x1")), "Wallet #2".to_string()), "vitalik.eth");
        assert_eq!(import_name(Some(&record("  ", "0x1")), "Wallet #2".to_string()), "Wallet #2");
        assert_eq!(import_name(None, "Wallet #2".to_string()), "Wallet #2");
    }

    const PHRASE: &str = "test test test test test test test test test test test junk";

    #[test]
    fn test_validate_import_phrase() {
        let words = |phrase: &str| phrase.split(' ').map(String::from).collect::<Vec<_>>();
        let validated = BsvHomeWalletImportType::SinglePhrase {
            words: vec![format!(" {PHRASE} ")],
            chain: Chain::Ethereum,
        }
        .validated()
        .unwrap();
        assert!(matches!(validated, BsvHomeWalletImportType::SinglePhrase { words: validated, .. } if validated == words(PHRASE)));

        assert_eq!(
            BsvHomeWalletImportType::MulticoinPhrase {
                words: words("test test test test test test test test test test test nope"),
                chains: vec![Chain::Ethereum],
            }
            .validated()
            .unwrap_err(),
            BsvHomeWalletImportError::InvalidSecretPhraseWords { words: vec!["nope".into()] }
        );
        assert_eq!(
            BsvHomeWalletImportType::MulticoinPhrase {
                words: words("test test test test test test test test test test test test"),
                chains: vec![Chain::Ethereum],
            }
            .validated()
            .unwrap_err(),
            BsvHomeWalletImportError::InvalidSecretPhrase
        );
    }

    #[test]
    fn test_validate_import_address_and_private_key() {
        let address = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
        let validated = BsvHomeWalletImportType::Address {
            address: format!(" {address} "),
            chain: Chain::Ethereum,
        }
        .validated()
        .unwrap();
        assert!(matches!(validated, BsvHomeWalletImportType::Address { address, .. } if address == "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));
        assert_eq!(
            BsvHomeWalletImportType::Address {
                address: "not an address".into(),
                chain: Chain::Ethereum,
            }
            .validated()
            .unwrap_err(),
            BsvHomeWalletImportError::InvalidAddress
        );
        assert_eq!(
            BsvHomeWalletImportType::PrivateKey {
                value: "zz".into(),
                chain: Chain::Ethereum,
            }
            .validated()
            .unwrap_err(),
            BsvHomeWalletImportError::InvalidPrivateKey
        );
        assert!(
            BsvHomeWalletImportType::PrivateKey {
                value: "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318".into(),
                chain: Chain::Ethereum,
            }
            .validated()
            .is_ok()
        );
    }

    fn wallet(id: WalletId, wallet_type: WalletType, chains: &[Chain]) -> Wallet {
        Wallet {
            id,
            wallet_type,
            ..Wallet::mock_with_accounts(Account::mock_chains(chains, "address"))
        }
    }

    #[test]
    fn test_secret_kind_follows_the_wallet_type() {
        let phrase = wallet(WalletId::Multicoin("0x1".to_string()), WalletType::Multicoin, &[Chain::Ethereum]);
        let private_key = wallet(WalletId::PrivateKey(Chain::Ethereum, "0x2".to_string()), WalletType::PrivateKey, &[Chain::Ethereum]);
        let view = wallet(WalletId::View(Chain::Ethereum, "0x3".to_string()), WalletType::View, &[Chain::Ethereum]);

        assert_eq!(secret_kind(&phrase), Some(BsvHomeWalletSecretKind::Phrase));
        assert_eq!(secret_kind(&private_key), Some(BsvHomeWalletSecretKind::PrivateKey));
        assert_eq!(secret_kind(&view), None);
    }

    #[test]
    fn test_row_comes_from_the_wallet_id() {
        let multicoin = row(&wallet(WalletId::Multicoin("0x1".to_string()), WalletType::Multicoin, &[Chain::Ethereum]));
        assert_eq!(multicoin.subtitle, BsvHomeWalletSubtitle::Multicoin);
        assert_eq!(multicoin.placeholder, BsvHomeWalletPlaceholder::Multicoin);
        assert!(!multicoin.shows_watch_badge);

        let view = row(&wallet(WalletId::View(Chain::Ethereum, "0x2".to_string()), WalletType::View, &[Chain::Ethereum]));
        assert_eq!(
            view.subtitle,
            BsvHomeWalletSubtitle::Account {
                chain: Chain::Ethereum,
                address: "0x2".to_string()
            }
        );
        assert_eq!(view.placeholder, BsvHomeWalletPlaceholder::Chain { chain: Chain::Ethereum });
        assert!(view.shows_watch_badge);

        let single = row(&wallet(WalletId::Single(Chain::Bitcoin, "bc1".to_string()), WalletType::Single, &[Chain::Bitcoin]));
        assert!(!single.shows_watch_badge);
    }

    #[test]
    fn test_wallets_missing_chains() {
        let multicoin = wallet(WalletId::Multicoin("0x1".to_string()), WalletType::Multicoin, &[Chain::Ethereum]);
        let complete = wallet(WalletId::Multicoin("0x2".to_string()), WalletType::Multicoin, &[Chain::Ethereum, Chain::Bitcoin]);
        let single = wallet(WalletId::Single(Chain::Ethereum, "0x3".to_string()), WalletType::Single, &[Chain::Ethereum]);

        let result = wallets_missing_chains(vec![multicoin.clone(), complete, single], &[Chain::Ethereum, Chain::Bitcoin]);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0.id, multicoin.id);
        assert_eq!(result[0].1, vec![Chain::Bitcoin]);
    }

    #[test]
    fn test_testnet_backfill_includes_single_mnemonic_wallets_without_expanding_private_or_view_wallets() {
        let multicoin = wallet(WalletId::Multicoin("multicoin".into()), WalletType::Multicoin, &[Chain::Tron]);
        let single = wallet(WalletId::Single(Chain::Tron, "single".into()), WalletType::Single, &[Chain::Tron]);
        let complete = wallet(WalletId::Single(Chain::Tron, "complete".into()), WalletType::Single, &[Chain::Tron, Chain::TronShasta]);
        let private = wallet(WalletId::PrivateKey(Chain::Tron, "private".into()), WalletType::PrivateKey, &[Chain::Tron]);
        let view = wallet(WalletId::View(Chain::Tron, "view".into()), WalletType::View, &[Chain::Tron]);
        let result = wallets_missing_chains(vec![multicoin.clone(), single.clone(), complete, private, view], &[Chain::Ethereum, Chain::TronShasta]);
        assert_eq!(
            result.into_iter().map(|(wallet, missing)| (wallet.id, missing)).collect::<Vec<_>>(),
            vec![(multicoin.id, vec![Chain::Ethereum, Chain::TronShasta]), (single.id, vec![Chain::TronShasta])]
        );
    }

    #[test]
    fn test_existing_wallet_requires_same_type() {
        let id = WalletId::Single(Chain::Bitcoin, "bc1".to_string());
        let wallets = vec![wallet(id.clone(), WalletType::Single, &[Chain::Bitcoin])];

        assert!(existing_wallet(&wallets, &id, WalletType::Single).is_some());
        assert!(existing_wallet(&wallets, &id, WalletType::PrivateKey).is_none());
        assert!(existing_wallet(&wallets, &WalletId::Multicoin("0x".to_string()), WalletType::Multicoin).is_none());
    }

    #[test]
    fn test_next_current_wallet_prefers_multicoin_then_lowest_index() {
        let mut view = wallet(WalletId::View(Chain::Ethereum, "0xv".to_string()), WalletType::View, &[Chain::Ethereum]);
        view.index = 0;
        let mut second = wallet(WalletId::Multicoin("0x2".to_string()), WalletType::Multicoin, &[Chain::Ethereum]);
        second.index = 2;
        let mut first = wallet(WalletId::Multicoin("0x1".to_string()), WalletType::Multicoin, &[Chain::Ethereum]);
        first.index = 1;

        assert_eq!(next_current_wallet(&[view, second, first.clone()]), Some(first.id));
        assert_eq!(next_current_wallet(&[]), None);
    }

    #[test]
    fn test_show_collections_follows_the_first_account_chain_outside_multicoin() {
        assert!(show_collections(WalletType::Multicoin, &[Chain::Bitcoin]));
        assert!(show_collections(WalletType::Single, &[Chain::Ethereum]));
        assert!(!show_collections(WalletType::Single, &[Chain::Bitcoin]));
        assert!(!show_collections(WalletType::View, &[]));
    }

    #[test]
    fn test_view_wallet() {
        let result = view_wallet("Watch".to_string(), Chain::Ethereum, "0xabc".to_string());
        assert_eq!(result.id, WalletId::View(Chain::Ethereum, "0xabc".to_string()));
        assert_eq!(result.wallet_type, WalletType::View);
        assert_eq!(result.accounts.len(), 1);
        assert_eq!(result.accounts[0].address, "0xabc");
    }

    #[test]
    fn test_next_wallet_index_uses_highest_index() {
        assert_eq!(next_wallet_index(&[]), 1);
        let mut first = wallet(WalletId::Multicoin("1".into()), WalletType::Multicoin, &[]);
        first.index = 3;
        let mut second = wallet(WalletId::Multicoin("2".into()), WalletType::Single, &[]);
        second.index = 1;
        assert_eq!(next_wallet_index(&[first, second]), 4);
    }

    #[test]
    fn test_wallets_sort_by_type_then_index() {
        let mut watch = wallet(WalletId::View(Chain::Ethereum, "0xv".to_string()), WalletType::View, &[Chain::Ethereum]);
        watch.index = 0;
        let mut second = wallet(WalletId::Multicoin("0x2".to_string()), WalletType::Multicoin, &[Chain::Ethereum]);
        second.index = 2;
        let mut first = wallet(WalletId::Multicoin("0x1".to_string()), WalletType::Multicoin, &[Chain::Ethereum]);
        first.index = 1;

        let sorted = sorted_wallets(vec![watch.clone(), second.clone(), first.clone()]);

        assert_eq!(sorted.iter().map(|wallet| wallet.id.clone()).collect::<Vec<_>>(), vec![first.id, second.id, watch.id]);
    }
}

#[cfg(test)]
mod hd_import_tests {
    use super::*;

    #[test]
    fn test_hd_import_routes_private_public_and_ordinary_imports() {
        let phrase = BsvHomeWalletImportKind::Phrase;
        let observer = BsvHomeWalletImportKind::Address;
        assert_eq!(bsv_hd_import_mode(phrase, None, "words", false), Some(BsvHomeBsvHdImportMode::Multicoin));
        assert_eq!(bsv_hd_import_mode(phrase, None, "words", true), Some(BsvHomeBsvHdImportMode::Recovery));
        assert_eq!(bsv_hd_import_mode(phrase, Some(Chain::BitcoinSv), "words", false), Some(BsvHomeBsvHdImportMode::Recovery));
        assert_eq!(bsv_hd_import_mode(phrase, None, "words::", false), Some(BsvHomeBsvHdImportMode::Recovery));
        assert_eq!(bsv_hd_import_mode(phrase, None, " {}", false), Some(BsvHomeBsvHdImportMode::Recovery));
        assert_eq!(bsv_hd_import_mode(phrase, Some(Chain::Ethereum), "words::", true), None);
        assert_eq!(bsv_hd_import_mode(observer, Some(Chain::BitcoinSv), " xpub", false), Some(BsvHomeBsvHdImportMode::Observer));
        assert_eq!(bsv_hd_import_mode(observer, Some(Chain::BitcoinSv), " {}", false), Some(BsvHomeBsvHdImportMode::Observer));
        assert_eq!(bsv_hd_import_mode(observer, Some(Chain::BitcoinSv), "address", false), None);
        assert_eq!(bsv_hd_import_mode(observer, Some(Chain::Ethereum), "xpub", false), None);
        assert_eq!(bsv_hd_import_mode(BsvHomeWalletImportKind::PrivateKey, Some(Chain::BitcoinSv), "key", true), None);
    }
}
