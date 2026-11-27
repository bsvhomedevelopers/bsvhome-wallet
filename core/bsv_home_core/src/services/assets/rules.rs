use std::str::FromStr;

use primitives::known_assets::{HYPERCORE_PERPETUAL_USDC, TRON_SHASTA_USDT};
use primitives::{
    Asset, AssetBasic, AssetId, AssetMetaData, AssetPrice, AssetProperties, AssetScore, BannerEvent, Chain, ConfigVersions, PerpetualProvider, PriceAlert, StakeChain,
    VerificationStatus, Wallet, WalletType,
};

use super::model::{
    AssetList, BsvHomeAssetAction, BsvHomeAssetDetailsState, BsvHomeAssetEmptyAction, BsvHomeAssetFilter, BsvHomeAssetNetworkDestination, BsvHomeAssetRow, BsvHomeAssetRowSubtitle,
    BsvHomeAssetRowTitle, BsvHomeAssetRowTrailing, BsvHomeHeaderActions, BsvHomeHeaderButton, BsvHomeHeaderButtonKind, BsvHomeSelectAssetFlow, BsvHomeSelectAssetScope,
    BsvHomeSelectAssetType, BsvHomeSelectRowAction, BsvHomeWalletSearchLimits,
};
use crate::config::search_config::{ASSETS_INITIAL_LIMIT, ASSETS_SEARCH_LIMIT, NFTS_PREVIEW_LIMIT, PERPETUALS_PREVIEW_LIMIT, RESULTS_LIMIT};
use crate::models::custom_types::BsvHomeBigUint;
use crate::perpetual::BsvHomePerpetual;
use crate::services::balance::BsvHomeAssetBalance;
use crate::services::balance::rules::is_required_asset;
use crate::services::bsv_cold_wallet::bsv_cold_wallet_can_send;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::nft::rules::nft_chains;
use crate::services::portal::BsvHomePortalSwapDirection;
use crate::services::price::rules::has_price;
use crate::services::price_alert::rules::displayed_price_alert_ids;
use swapper::AssetList as SwapAssetList;

use crate::models::asset::{wallet_asset_is_enabled, wallet_default_assets};
use crate::services::collections::{missing, missing_by, unique};

pub fn asset_list_versions(versions: &ConfigVersions) -> [(AssetList, i32); 3] {
    [
        (AssetList::Buy, versions.fiat_on_ramp_assets),
        (AssetList::Sell, versions.fiat_off_ramp_assets),
        (AssetList::Swap, versions.swap_assets),
    ]
}

pub fn is_asset_list_outdated(stored_version: Option<&str>, remote_version: i32) -> bool {
    stored_version != Some(remote_version.to_string().as_str())
}

pub fn asset_ids(ids: &[String]) -> Vec<AssetId> {
    ids.iter().filter_map(|id| AssetId::new(id)).collect()
}

pub fn swappable_chain_asset_ids() -> Vec<AssetId> {
    Chain::all().into_iter().filter(Chain::is_swap_supported).map(AssetId::from_chain).collect()
}

pub fn token_search_chains(chains: &[Chain]) -> Vec<Chain> {
    if chains.is_empty() { Chain::all() } else { chains.to_vec() }
}

pub fn missing_asset_ids(requested: Vec<AssetId>, existing: Vec<AssetId>) -> Vec<AssetId> {
    missing(requested, existing)
}

pub fn asset_prices(assets: &[AssetBasic]) -> Vec<AssetPrice> {
    assets
        .iter()
        .filter_map(|asset| {
            asset
                .price
                .as_ref()
                .map(|price| AssetPrice::new(asset.asset.id.clone(), price.price, price.price_change_percentage_24h, price.updated_at))
        })
        .collect()
}

pub fn default_asset_basic(asset: Asset) -> AssetBasic {
    let asset_id = asset.id.clone();
    AssetBasic::new(asset, AssetProperties::default(asset_id.clone()), AssetScore::new(asset_id.default_rank()))
}

pub fn default_assets() -> Vec<AssetBasic> {
    Chain::all()
        .into_iter()
        .flat_map(|chain| std::iter::once(Asset::from_chain(chain)).chain(wallet_default_assets(chain)))
        .map(default_asset_basic)
        .collect()
}

pub fn missing_assets(assets: Vec<AssetBasic>, existing: Vec<AssetId>) -> Vec<AssetBasic> {
    missing_by(assets, existing, |asset| asset.asset.id.clone())
}

pub fn stakeable_asset_ids() -> Vec<AssetId> {
    Chain::all().into_iter().filter(Chain::is_stake_supported).map(AssetId::from_chain).collect()
}

pub fn default_token_chain(chains: &[Chain]) -> Option<Chain> {
    chains.iter().find(|chain| **chain == Chain::Ethereum).or(chains.first()).copied()
}

pub fn token_chains(wallet: &Wallet) -> Vec<Chain> {
    let mut chains = unique(wallet.accounts.iter().map(|account| account.chain).filter(|chain| chain.default_asset_type().is_some()));
    chains.sort_by_key(|chain| std::cmp::Reverse(AssetId::from_chain(*chain).default_rank()));
    chains
}

pub(crate) fn local_asset(asset_id: &AssetId) -> Option<Asset> {
    if *asset_id == Chain::BitcoinSv.as_asset_id() || *asset_id == Chain::TronShasta.as_asset_id() {
        Some(Asset::from_chain(asset_id.chain))
    } else if *asset_id == TRON_SHASTA_USDT.id {
        Some(TRON_SHASTA_USDT.clone())
    } else {
        None
    }
}

pub fn popular_asset_ids() -> Vec<AssetId> {
    [Chain::Bitcoin, Chain::Ethereum, Chain::Solana].into_iter().map(AssetId::from_chain).collect()
}

pub fn can_open(wallet: &Wallet, asset_id: &AssetId) -> bool {
    (asset_id.is_token() || asset_id.chain.has_native_asset()) && wallet.account(asset_id.chain).is_some()
}

pub fn default_balances(wallet: &Wallet) -> (Vec<AssetId>, Vec<AssetId>) {
    unique(wallet.accounts.iter().flat_map(|account| {
        let chain = account.chain;
        let native = (chain.rank() >= 0).then(|| AssetId::from_chain(chain));
        native.into_iter().chain(wallet_default_assets(chain).into_iter().map(|asset| asset.id))
    }))
    .into_iter()
    .partition(|asset_id| wallet_asset_is_enabled(asset_id.clone(), wallet.wallet_type))
}

pub fn network_destination(asset_id: &AssetId) -> Option<BsvHomeAssetNetworkDestination> {
    let chain = asset_id.chain;
    if asset_id.is_token() && chain.has_native_asset() {
        return Some(BsvHomeAssetNetworkDestination::Asset { asset: Asset::from_chain(chain) });
    }
    chain.default_asset_type().is_some().then_some(BsvHomeAssetNetworkDestination::Assets { chain })
}

pub fn verification_status(asset: &Asset, rank: i32) -> Option<VerificationStatus> {
    if asset.id.is_native() {
        return None;
    }
    match VerificationStatus::from_rank(rank) {
        VerificationStatus::Unverified => Some(VerificationStatus::Unverified),
        VerificationStatus::Verified | VerificationStatus::Suspicious => None,
    }
}

fn with_filter(mut flow: BsvHomeSelectAssetFlow, filter: Option<BsvHomeAssetFilter>) -> BsvHomeSelectAssetFlow {
    flow.filters.extend(filter);
    flow
}

pub fn select_asset_flow(select_type: BsvHomeSelectAssetType, swap_receive_assets: Option<SwapAssetList>) -> BsvHomeSelectAssetFlow {
    let row = |shows_symbol: bool, subtitle: BsvHomeAssetRowSubtitle, trailing: BsvHomeAssetRowTrailing| BsvHomeAssetRow {
        title: BsvHomeAssetRowTitle::Asset,
        shows_symbol,
        subtitle,
        trailing,
    };
    let flow = |row_action: BsvHomeSelectRowAction, action: Option<BsvHomeAssetAction>| BsvHomeSelectAssetFlow {
        row: row(false, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Balance),
        row_action,
        action,
        scope: BsvHomeSelectAssetScope::Wallet,
        filters: action.map(|action| action.filters()).unwrap_or_default(),
        enables_price_alert: false,
        network_search: false,
        chain_filter: false,
        recents: false,
        popular_section: false,
        balance_filter: false,
        add_custom_token: false,
        deposit_asset_display: false,
    };
    match select_type {
        BsvHomeSelectAssetType::Send => BsvHomeSelectAssetFlow {
            chain_filter: true,
            recents: true,
            ..flow(BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Send))
        },
        BsvHomeSelectAssetType::Receive => BsvHomeSelectAssetFlow {
            row: row(true, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Copy),
            network_search: true,
            chain_filter: true,
            recents: true,
            ..flow(BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Receive))
        },
        BsvHomeSelectAssetType::ReceiveCollection => with_filter(
            BsvHomeSelectAssetFlow {
                row: BsvHomeAssetRow {
                    title: BsvHomeAssetRowTitle::Network,
                    ..row(false, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Copy)
                },
                network_search: true,
                recents: true,
                ..flow(BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Receive))
            },
            Some(BsvHomeAssetFilter::asset_ids(nft_chains().into_iter().map(AssetId::from_chain).collect())),
        ),
        BsvHomeSelectAssetType::Buy => BsvHomeSelectAssetFlow {
            network_search: true,
            chain_filter: true,
            recents: true,
            popular_section: true,
            ..flow(BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Buy))
        },
        BsvHomeSelectAssetType::SwapPay => BsvHomeSelectAssetFlow {
            chain_filter: true,
            recents: true,
            ..flow(BsvHomeSelectRowAction::Select, Some(BsvHomeAssetAction::SwapPay))
        },
        BsvHomeSelectAssetType::SwapReceive { .. } => with_filter(
            BsvHomeSelectAssetFlow {
                network_search: true,
                chain_filter: true,
                recents: true,
                ..flow(BsvHomeSelectRowAction::Select, Some(BsvHomeAssetAction::SwapReceive))
            },
            swap_receive_assets.map(BsvHomeAssetFilter::from),
        ),
        BsvHomeSelectAssetType::Manage => with_filter(
            BsvHomeSelectAssetFlow {
                row: row(true, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Toggle),
                network_search: true,
                chain_filter: true,
                balance_filter: true,
                add_custom_token: true,
                ..flow(BsvHomeSelectRowAction::Toggle, None)
            },
            Some(BsvHomeAssetFilter::Enabled),
        ),
        BsvHomeSelectAssetType::PriceAlert => with_filter(
            BsvHomeSelectAssetFlow {
                row: row(true, BsvHomeAssetRowSubtitle::Price, BsvHomeAssetRowTrailing::None),
                enables_price_alert: true,
                network_search: true,
                chain_filter: true,
                popular_section: true,
                scope: BsvHomeSelectAssetScope::AllAssets,
                ..flow(BsvHomeSelectRowAction::Select, None)
            },
            Some(BsvHomeAssetFilter::Enabled),
        ),
        BsvHomeSelectAssetType::Deposit => with_filter(
            flow(BsvHomeSelectRowAction::Navigate, None),
            Some(BsvHomeAssetFilter::asset_ids(vec![BsvHomePerpetual::new(PerpetualProvider::Hypercore).deposit_asset().id])),
        ),
        BsvHomeSelectAssetType::Withdraw => with_filter(
            BsvHomeSelectAssetFlow {
                deposit_asset_display: true,
                ..flow(BsvHomeSelectRowAction::Navigate, None)
            },
            Some(BsvHomeAssetFilter::asset_ids(vec![HYPERCORE_PERPETUAL_USDC.id.clone()])),
        ),
        BsvHomeSelectAssetType::WalletSearch => BsvHomeSelectAssetFlow {
            row: row(false, BsvHomeAssetRowSubtitle::Price, BsvHomeAssetRowTrailing::Balance),
            network_search: true,
            recents: true,
            ..flow(BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Open))
        },
        BsvHomeSelectAssetType::WalletSearchResults => BsvHomeSelectAssetFlow {
            row: row(false, BsvHomeAssetRowSubtitle::Price, BsvHomeAssetRowTrailing::Balance),
            ..flow(BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Open))
        },
    }
}

pub fn select_asset_flow_with_environment(
    select_type: BsvHomeSelectAssetType,
    swap_receive_assets: Option<SwapAssetList>,
    environment: BsvHomeRuntimeEnvironment,
) -> BsvHomeSelectAssetFlow {
    let mut flow = select_asset_flow(select_type, swap_receive_assets);
    if let Some(asset_ids) = environment.allowed_asset_ids() {
        flow.filters.push(BsvHomeAssetFilter::asset_ids(asset_ids));
        flow.add_custom_token = false;
        flow.popular_section = false;
    }
    flow
}

pub fn wallet_search_limits(query: &str) -> BsvHomeWalletSearchLimits {
    let assets = match query.trim().is_empty() {
        true => ASSETS_INITIAL_LIMIT,
        false => ASSETS_SEARCH_LIMIT,
    };
    BsvHomeWalletSearchLimits {
        assets,
        fetch: assets + 1,
        perpetuals: PERPETUALS_PREVIEW_LIMIT,
        nfts: NFTS_PREVIEW_LIMIT,
        results: RESULTS_LIMIT,
    }
}

pub(crate) fn bsv_wallet_header_actions(wallet: &Wallet, chain: Chain, banner_events: &[BannerEvent], environment: BsvHomeRuntimeEnvironment) -> Option<BsvHomeHeaderActions> {
    cold_wallet_header_actions(wallet, chain, banner_events).or_else(|| wowpay_watch_receive_actions(wallet, chain, banner_events, environment))
}

pub(crate) fn wowpay_watch_receive_actions(wallet: &Wallet, chain: Chain, banner_events: &[BannerEvent], environment: BsvHomeRuntimeEnvironment) -> Option<BsvHomeHeaderActions> {
    if environment != BsvHomeRuntimeEnvironment::WowPay || wallet.wallet_type != WalletType::View || chain != Chain::BitcoinSv || wallet.account(chain).is_none() {
        return None;
    }
    Some(BsvHomeHeaderActions::Buttons {
        buttons: vec![BsvHomeHeaderButton {
            kind: BsvHomeHeaderButtonKind::Receive,
            is_enabled: !banner_events
                .iter()
                .any(|event| *event == BannerEvent::ActivateAsset || *event == BannerEvent::AccountBlockedMultiSignature),
        }],
    })
}

pub(crate) fn cold_wallet_header_actions(wallet: &Wallet, chain: Chain, banner_events: &[BannerEvent]) -> Option<BsvHomeHeaderActions> {
    if chain != Chain::BitcoinSv || !bsv_cold_wallet_can_send(wallet.clone()) {
        return None;
    }
    let is_enabled = !banner_events
        .iter()
        .any(|event| *event == BannerEvent::ActivateAsset || *event == BannerEvent::AccountBlockedMultiSignature);
    Some(BsvHomeHeaderActions::Buttons {
        buttons: [BsvHomeHeaderButtonKind::Send, BsvHomeHeaderButtonKind::Receive]
            .into_iter()
            .map(|kind| BsvHomeHeaderButton { kind, is_enabled })
            .collect(),
    })
}

pub fn details_state(
    wallet_type: WalletType,
    chain: Chain,
    metadata: &AssetMetaData,
    balance: &BsvHomeAssetBalance,
    banner_events: &[BannerEvent],
    price: Option<f64>,
    price_alerts: Vec<PriceAlert>,
) -> BsvHomeAssetDetailsState {
    let is_view_only = wallet_type == WalletType::View;
    let market_features_enabled = !chain.is_testnet();
    let is_buy_enabled = market_features_enabled && metadata.is_buy_enabled;
    let is_swap_enabled = market_features_enabled && metadata.is_swap_enabled && balance.asset_id != Chain::Tron.as_asset_id();
    let buttons_enabled = !banner_events
        .iter()
        .any(|event| matches!(event, BannerEvent::ActivateAsset | BannerEvent::AccountBlockedMultiSignature));
    let button = |kind: BsvHomeHeaderButtonKind, shows: bool| {
        shows.then_some(BsvHomeHeaderButton {
            kind,
            is_enabled: buttons_enabled,
        })
    };
    BsvHomeAssetDetailsState {
        is_view_only,
        header_actions: if is_view_only {
            BsvHomeHeaderActions::WatchOnly
        } else {
            BsvHomeHeaderActions::Buttons {
                buttons: [
                    button(BsvHomeHeaderButtonKind::Send, true),
                    button(BsvHomeHeaderButtonKind::Receive, true),
                    button(BsvHomeHeaderButtonKind::Buy, is_buy_enabled),
                    button(
                        BsvHomeHeaderButtonKind::Swap,
                        is_swap_enabled || market_features_enabled && BsvHomePortalSwapDirection::for_asset(balance.asset_id.clone()).is_some(),
                    ),
                ]
                .into_iter()
                .flatten()
                .collect(),
            }
        },
        shows_banners: !banner_events.is_empty(),
        shows_manage: !metadata.is_balance_enabled && !is_required_asset(&balance.asset_id),
        shows_resources: market_features_enabled && StakeChain::from_str(chain.as_ref()).is_ok_and(|stake_chain| stake_chain.get_uses_freeze()),
        shows_price_alerts: market_features_enabled && has_price(price) && !displayed_price_alert_ids(price_alerts).is_empty(),
        shows_price_change: market_features_enabled
            && crate::services::price::rules::defillama_coin_id(&balance.asset_id).is_none_or(|_| primitives::PriceProvider::DefiLlama.supports_price_change_24h()),
        shows_earn: market_features_enabled && metadata.is_earn_enabled && !is_view_only && balance.earn == BsvHomeBigUint::ZERO,
        empty_transactions_action: if is_buy_enabled {
            Some(BsvHomeAssetEmptyAction::Buy)
        } else if is_swap_enabled {
            Some(BsvHomeAssetEmptyAction::Swap)
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_each_select_flow_decides_its_row_action_and_recent_activity() {
        let row = |select_type: BsvHomeSelectAssetType| (select_type.flow().row_action, select_type.flow().action);
        assert_eq!(row(BsvHomeSelectAssetType::Send), (BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Send)));
        assert_eq!(row(BsvHomeSelectAssetType::Receive), (BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Receive)));
        assert_eq!(
            row(BsvHomeSelectAssetType::ReceiveCollection),
            (BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Receive))
        );
        assert_eq!(row(BsvHomeSelectAssetType::Buy), (BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Buy)));
        assert_eq!(row(BsvHomeSelectAssetType::SwapPay), (BsvHomeSelectRowAction::Select, Some(BsvHomeAssetAction::SwapPay)));
        assert_eq!(
            row(BsvHomeSelectAssetType::SwapReceive { pay_asset_id: None }),
            (BsvHomeSelectRowAction::Select, Some(BsvHomeAssetAction::SwapReceive))
        );
        assert_eq!(row(BsvHomeSelectAssetType::Manage), (BsvHomeSelectRowAction::Toggle, None));
        assert_eq!(row(BsvHomeSelectAssetType::PriceAlert), (BsvHomeSelectRowAction::Select, None));
        assert_eq!(row(BsvHomeSelectAssetType::Deposit), (BsvHomeSelectRowAction::Navigate, None));
        assert_eq!(row(BsvHomeSelectAssetType::Withdraw), (BsvHomeSelectRowAction::Navigate, None));
        assert_eq!(
            row(BsvHomeSelectAssetType::WalletSearch),
            (BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Open))
        );
        assert_eq!(
            row(BsvHomeSelectAssetType::WalletSearchResults),
            (BsvHomeSelectRowAction::Navigate, Some(BsvHomeAssetAction::Open))
        );
    }

    #[test]
    fn test_each_select_flow_shows_the_row_both_apps_draw() {
        let row = |select_type: BsvHomeSelectAssetType| {
            let row = select_type.flow().row;
            (row.title, row.shows_symbol, row.subtitle, row.trailing)
        };
        let balance = (BsvHomeAssetRowTitle::Asset, false, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Balance);
        for select_type in [
            BsvHomeSelectAssetType::Send,
            BsvHomeSelectAssetType::Buy,
            BsvHomeSelectAssetType::SwapPay,
            BsvHomeSelectAssetType::SwapReceive { pay_asset_id: None },
            BsvHomeSelectAssetType::Deposit,
            BsvHomeSelectAssetType::Withdraw,
        ] {
            assert_eq!(row(select_type), balance);
        }
        assert_eq!(
            row(BsvHomeSelectAssetType::Receive),
            (BsvHomeAssetRowTitle::Asset, true, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Copy)
        );
        assert_eq!(
            row(BsvHomeSelectAssetType::ReceiveCollection),
            (BsvHomeAssetRowTitle::Network, false, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Copy)
        );
        assert_eq!(
            row(BsvHomeSelectAssetType::Manage),
            (BsvHomeAssetRowTitle::Asset, true, BsvHomeAssetRowSubtitle::Network, BsvHomeAssetRowTrailing::Toggle)
        );
        assert_eq!(
            row(BsvHomeSelectAssetType::PriceAlert),
            (BsvHomeAssetRowTitle::Asset, true, BsvHomeAssetRowSubtitle::Price, BsvHomeAssetRowTrailing::None)
        );
        let search = (BsvHomeAssetRowTitle::Asset, false, BsvHomeAssetRowSubtitle::Price, BsvHomeAssetRowTrailing::Balance);
        assert_eq!(row(BsvHomeSelectAssetType::WalletSearch), search);
        assert_eq!(row(BsvHomeSelectAssetType::WalletSearchResults), search);
    }

    #[test]
    fn test_each_select_flow_enables_only_its_capabilities() {
        let enabled = |select_type: BsvHomeSelectAssetType| {
            let flow = select_type.flow();
            [
                ("network_search", flow.network_search),
                ("chain_filter", flow.chain_filter),
                ("recents", flow.recents),
                ("popular_section", flow.popular_section),
                ("balance_filter", flow.balance_filter),
                ("add_custom_token", flow.add_custom_token),
                ("deposit_asset_display", flow.deposit_asset_display),
                ("enables_price_alert", flow.enables_price_alert),
            ]
            .into_iter()
            .filter_map(|(name, on)| on.then_some(name))
            .collect::<Vec<_>>()
        };
        assert_eq!(enabled(BsvHomeSelectAssetType::Send), ["chain_filter", "recents"]);
        assert_eq!(enabled(BsvHomeSelectAssetType::Receive), ["network_search", "chain_filter", "recents"]);
        assert_eq!(enabled(BsvHomeSelectAssetType::ReceiveCollection), ["network_search", "recents"]);
        assert_eq!(enabled(BsvHomeSelectAssetType::Buy), ["network_search", "chain_filter", "recents", "popular_section"]);
        assert_eq!(enabled(BsvHomeSelectAssetType::SwapPay), ["chain_filter", "recents"]);
        assert_eq!(
            enabled(BsvHomeSelectAssetType::SwapReceive { pay_asset_id: None }),
            ["network_search", "chain_filter", "recents"]
        );
        assert_eq!(
            enabled(BsvHomeSelectAssetType::Manage),
            ["network_search", "chain_filter", "balance_filter", "add_custom_token"]
        );
        assert_eq!(
            enabled(BsvHomeSelectAssetType::PriceAlert),
            ["network_search", "chain_filter", "popular_section", "enables_price_alert"]
        );
        assert!(enabled(BsvHomeSelectAssetType::Deposit).is_empty());
        assert_eq!(enabled(BsvHomeSelectAssetType::Withdraw), ["deposit_asset_display"]);
        assert_eq!(enabled(BsvHomeSelectAssetType::WalletSearch), ["network_search", "recents"]);
        assert!(enabled(BsvHomeSelectAssetType::WalletSearchResults).is_empty());
    }

    #[test]
    fn test_each_select_flow_lists_the_rows_its_action_can_offer() {
        let rows = |select_type: BsvHomeSelectAssetType| (select_type.flow().scope, select_type.flow().filters);
        assert_eq!(
            rows(BsvHomeSelectAssetType::Send),
            (BsvHomeSelectAssetScope::Wallet, vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::HasBalance])
        );
        assert_eq!(rows(BsvHomeSelectAssetType::Receive), (BsvHomeSelectAssetScope::Wallet, vec![BsvHomeAssetFilter::Enabled]));
        let (_, collection_filters) = rows(BsvHomeSelectAssetType::ReceiveCollection);
        assert_eq!(collection_filters[0], BsvHomeAssetFilter::Enabled);
        let BsvHomeAssetFilter::ChainsOrAssetIds {
            chains: collection_chains,
            asset_ids: collection_assets,
        } = &collection_filters[1]
        else {
            panic!("receive collection lists the NFT chains' native assets");
        };
        assert!(collection_chains.is_empty());
        assert!(collection_assets.contains(&AssetId::from_chain(Chain::Ethereum)));
        assert!(!collection_assets.contains(&AssetId::from_chain(Chain::Bitcoin)));
        assert!(collection_assets.iter().all(AssetId::is_native));
        assert_eq!(
            rows(BsvHomeSelectAssetType::Buy),
            (BsvHomeSelectAssetScope::Wallet, vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Buyable])
        );
        assert_eq!(
            rows(BsvHomeSelectAssetType::SwapPay),
            (
                BsvHomeSelectAssetScope::Wallet,
                vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Swappable, BsvHomeAssetFilter::HasAvailableBalance]
            )
        );
        assert_eq!(
            rows(BsvHomeSelectAssetType::SwapReceive { pay_asset_id: None }),
            (BsvHomeSelectAssetScope::Wallet, vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Swappable])
        );
        assert_eq!(rows(BsvHomeSelectAssetType::Manage), (BsvHomeSelectAssetScope::Wallet, vec![BsvHomeAssetFilter::Enabled]));
        assert_eq!(
            rows(BsvHomeSelectAssetType::PriceAlert),
            (BsvHomeSelectAssetScope::AllAssets, vec![BsvHomeAssetFilter::Enabled])
        );
        assert_eq!(
            rows(BsvHomeSelectAssetType::Deposit),
            (
                BsvHomeSelectAssetScope::Wallet,
                vec![BsvHomeAssetFilter::ChainsOrAssetIds {
                    chains: Vec::new(),
                    asset_ids: vec![AssetId::from_token(Chain::Arbitrum, "0xaf88d065e77c8cC2239327C5EDb3A432268e5831")]
                }]
            )
        );
        assert_eq!(
            rows(BsvHomeSelectAssetType::Withdraw),
            (
                BsvHomeSelectAssetScope::Wallet,
                vec![BsvHomeAssetFilter::ChainsOrAssetIds {
                    chains: Vec::new(),
                    asset_ids: vec![HYPERCORE_PERPETUAL_USDC.id.clone()]
                }]
            )
        );
        assert_eq!(rows(BsvHomeSelectAssetType::WalletSearch), (BsvHomeSelectAssetScope::Wallet, Vec::new()));
        assert_eq!(rows(BsvHomeSelectAssetType::WalletSearchResults), (BsvHomeSelectAssetScope::Wallet, Vec::new()));
    }

    #[test]
    fn test_wowpay_select_flows_intersect_exact_assets_without_custom_token_extensions() {
        let allowed = BsvHomeRuntimeEnvironment::WowPay.allowed_asset_ids().unwrap();
        let select_types = [
            BsvHomeSelectAssetType::Send,
            BsvHomeSelectAssetType::Receive,
            BsvHomeSelectAssetType::ReceiveCollection,
            BsvHomeSelectAssetType::Buy,
            BsvHomeSelectAssetType::SwapPay,
            BsvHomeSelectAssetType::SwapReceive { pay_asset_id: None },
            BsvHomeSelectAssetType::Manage,
            BsvHomeSelectAssetType::PriceAlert,
            BsvHomeSelectAssetType::Deposit,
            BsvHomeSelectAssetType::Withdraw,
            BsvHomeSelectAssetType::WalletSearch,
            BsvHomeSelectAssetType::WalletSearchResults,
        ];
        for select_type in select_types {
            let unrestricted = select_asset_flow(select_type.clone(), None);
            assert_eq!(
                select_asset_flow_with_environment(select_type.clone(), None, BsvHomeRuntimeEnvironment::Production),
                unrestricted
            );
            let restricted = select_asset_flow_with_environment(select_type, None, BsvHomeRuntimeEnvironment::WowPay);
            assert_eq!(restricted.filters.last(), Some(&BsvHomeAssetFilter::asset_ids(allowed.clone())));
            assert!(!restricted.add_custom_token);
            assert!(!restricted.popular_section);
            assert_eq!(restricted.row_action, unrestricted.row_action);
            assert_eq!(restricted.action, unrestricted.action);
        }
    }

    #[test]
    fn test_swap_receive_lists_the_pay_assets_swap_universe() {
        let pay_asset_id = AssetId::from_chain(Chain::Ethereum);
        let universe = SwapAssetList {
            chains: vec![Chain::Solana],
            asset_ids: vec![AssetId::from_token(Chain::SmartChain, "0x123")],
        };

        let flow = select_asset_flow(BsvHomeSelectAssetType::SwapReceive { pay_asset_id: Some(pay_asset_id) }, Some(universe.clone()));

        assert_eq!(
            flow.filters,
            vec![
                BsvHomeAssetFilter::Enabled,
                BsvHomeAssetFilter::Swappable,
                BsvHomeAssetFilter::ChainsOrAssetIds {
                    chains: universe.chains,
                    asset_ids: universe.asset_ids,
                },
            ]
        );
    }

    #[test]
    fn test_wallet_search_limits_widen_while_searching_and_fetch_one_more_than_shown() {
        let initial = wallet_search_limits("  ");
        let searching = wallet_search_limits("btc");

        assert_eq!((initial.assets, initial.fetch), (12, 13));
        assert_eq!((searching.assets, searching.fetch), (25, 26));
        assert_eq!((initial.perpetuals, initial.nfts, initial.results), (3, 3, 100));
        assert_eq!((searching.perpetuals, searching.nfts, searching.results), (3, 3, 100));
    }

    #[test]
    fn test_network_destination_opens_the_native_asset_or_the_chain_list() {
        let ethereum = Chain::Ethereum;
        assert_eq!(
            network_destination(&Asset::mock_ethereum_usdc().id),
            Some(BsvHomeAssetNetworkDestination::Asset {
                asset: Asset::from_chain(ethereum)
            })
        );
        assert_eq!(
            network_destination(&AssetId::from_token(Chain::Tempo, "0x1")),
            Some(BsvHomeAssetNetworkDestination::Assets { chain: Chain::Tempo })
        );
        assert_eq!(
            network_destination(&AssetId::from_chain(ethereum)),
            Some(BsvHomeAssetNetworkDestination::Assets { chain: ethereum })
        );
        assert_eq!(network_destination(&AssetId::from_chain(Chain::Bitcoin)), None);
    }

    #[test]
    fn test_verification_status_rows_unverified_tokens_only() {
        let token = Asset::mock_ethereum_usdc();
        assert_eq!(verification_status(&token, 10), Some(VerificationStatus::Unverified));
        assert_eq!(verification_status(&token, 3), None);
        assert_eq!(verification_status(&token, 20), None);
        assert_eq!(verification_status(&Asset::mock(), 10), None);
    }

    #[test]
    fn test_default_token_chain_prefers_ethereum_then_first() {
        assert_eq!(default_token_chain(&[Chain::Solana, Chain::Ethereum]), Some(Chain::Ethereum));
        assert_eq!(default_token_chain(&[Chain::Solana, Chain::Tron]), Some(Chain::Solana));
        assert_eq!(default_token_chain(&[]), None);
    }

    #[test]
    fn test_token_chains_keeps_token_networks_by_rank() {
        let multicoin = wallet(WalletType::Multicoin, &[Chain::Bitcoin, Chain::Doge, Chain::Near, Chain::Xrp, Chain::Ethereum, Chain::Near]);
        assert_eq!(token_chains(&multicoin), vec![Chain::Ethereum, Chain::Xrp, Chain::Near]);
        assert!(token_chains(&wallet(WalletType::Single, &[Chain::Bitcoin])).is_empty());
    }

    #[test]
    fn test_popular_asset_ids_are_distinct_native_assets() {
        let ids = popular_asset_ids();

        assert!(!ids.is_empty());
        assert!(ids.iter().all(|id| id.is_native() && id.chain.has_native_asset()));
        assert_eq!(unique(ids.clone()).len(), ids.len());
        assert_eq!(ids.first(), Some(&AssetId::from_chain(Chain::Bitcoin)));
    }

    #[test]
    fn test_can_open_requires_account_and_native_asset() {
        let wallet = wallet(WalletType::Multicoin, &[Chain::Ethereum, Chain::Tempo]);
        assert!(can_open(&wallet, &AssetId::from_chain(Chain::Ethereum)));
        assert!(can_open(
            &wallet,
            &AssetId::from(Chain::Ethereum, Some("0xdac17f958d2ee523a2206206994597c13d831ec7".to_string()))
        ));
        assert!(!can_open(&wallet, &AssetId::from_chain(Chain::Bitcoin)));
        assert!(!can_open(&wallet, &AssetId::from_chain(Chain::Tempo)));
        assert!(can_open(
            &wallet,
            &AssetId::from(Chain::Tempo, Some("0x20c000000000000000000000c48d6a3bd5b7b0c2".to_string()))
        ));
    }

    #[test]
    fn test_default_assets_and_missing() {
        let assets = default_assets();
        let bitcoin = AssetId::from_chain(Chain::Bitcoin);
        let tron_usdt = wallet_default_assets(Chain::Tron)[0].id.clone();
        assert!(assets.iter().any(|asset| asset.asset.id == bitcoin));
        assert!(assets.iter().any(|asset| asset.asset.id == tron_usdt));

        let missing = missing_assets(assets.clone(), vec![bitcoin.clone()]);
        assert_eq!(missing.len(), assets.len() - 1);
        assert!(!missing.iter().any(|asset| asset.asset.id == bitcoin));
        assert!(stakeable_asset_ids().contains(&AssetId::from_chain(Chain::Cosmos)));
        assert!(!stakeable_asset_ids().contains(&bitcoin));
    }
    use bsv_home_derivation::{bsv_account_xpub, derive_accounts_from_mnemonic};
    use chrono::Utc;
    use primitives::testkit::ABANDON_PHRASE;
    use primitives::{Account, Chain, PriceAlertDirection, WalletType, currency::Currency};

    fn wallet(wallet_type: WalletType, chains: &[Chain]) -> Wallet {
        Wallet {
            wallet_type,
            ..Wallet::mock_with_accounts(Account::mock_chains(chains, "address"))
        }
    }

    #[test]
    fn test_cold_observer_header_actions_require_valid_bsv_pairing_and_respect_blocked_accounts() {
        let mut account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
        account.extended_public_key = Some(bsv_account_xpub(ABANDON_PHRASE).unwrap());
        let mut observer = wallet(WalletType::View, &[Chain::BitcoinSv]);
        observer.accounts = vec![account];
        let actions = |enabled| {
            Some(BsvHomeHeaderActions::Buttons {
                buttons: vec![
                    BsvHomeHeaderButton {
                        kind: BsvHomeHeaderButtonKind::Send,
                        is_enabled: enabled,
                    },
                    BsvHomeHeaderButton {
                        kind: BsvHomeHeaderButtonKind::Receive,
                        is_enabled: enabled,
                    },
                ],
            })
        };
        assert_eq!(cold_wallet_header_actions(&observer, Chain::BitcoinSv, &[]), actions(true));
        assert_eq!(
            cold_wallet_header_actions(&observer, Chain::BitcoinSv, &[BannerEvent::AccountBlockedMultiSignature]),
            actions(false)
        );
        assert_eq!(cold_wallet_header_actions(&observer, Chain::Tron, &[]), None);
        observer.accounts[0].extended_public_key = None;
        assert_eq!(cold_wallet_header_actions(&observer, Chain::BitcoinSv, &[]), None);
        observer.accounts[0].extended_public_key = Some("xpub-invalid".into());
        assert_eq!(cold_wallet_header_actions(&observer, Chain::BitcoinSv, &[]), None);
    }

    #[test]
    fn test_bsv_header_prefers_verified_observer_send_before_plain_watch_receive_only() {
        let mut account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
        account.extended_public_key = Some(bsv_account_xpub(ABANDON_PHRASE).unwrap());
        let mut observer = wallet(WalletType::View, &[Chain::BitcoinSv]);
        observer.accounts = vec![account];
        let button = |kind, is_enabled| BsvHomeHeaderButton { kind, is_enabled };
        let expected = |enabled| {
            Some(BsvHomeHeaderActions::Buttons {
                buttons: vec![button(BsvHomeHeaderButtonKind::Send, enabled), button(BsvHomeHeaderButtonKind::Receive, enabled)],
            })
        };
        assert_eq!(
            bsv_wallet_header_actions(&observer, Chain::BitcoinSv, &[], BsvHomeRuntimeEnvironment::WowPay),
            expected(true)
        );
        assert_eq!(
            bsv_wallet_header_actions(&observer, Chain::BitcoinSv, &[BannerEvent::AccountBlockedMultiSignature], BsvHomeRuntimeEnvironment::WowPay),
            expected(false)
        );
        observer.accounts[0].extended_public_key = None;
        assert_eq!(
            bsv_wallet_header_actions(&observer, Chain::BitcoinSv, &[], BsvHomeRuntimeEnvironment::WowPay),
            Some(BsvHomeHeaderActions::Buttons {
                buttons: vec![button(BsvHomeHeaderButtonKind::Receive, true)]
            })
        );
    }

    #[test]
    fn test_wowpay_watch_receive_never_authorizes_sending_or_hidden_chains() {
        let watch = wallet(WalletType::View, &[Chain::BitcoinSv]);
        for events in [vec![], vec![BannerEvent::ActivateAsset], vec![BannerEvent::AccountBlockedMultiSignature]] {
            assert_eq!(
                wowpay_watch_receive_actions(&watch, Chain::BitcoinSv, &events, BsvHomeRuntimeEnvironment::WowPay),
                Some(BsvHomeHeaderActions::Buttons {
                    buttons: vec![BsvHomeHeaderButton {
                        kind: BsvHomeHeaderButtonKind::Receive,
                        is_enabled: events.is_empty()
                    }]
                })
            );
        }
        assert_eq!(wowpay_watch_receive_actions(&watch, Chain::BitcoinSv, &[], BsvHomeRuntimeEnvironment::Production), None);
        assert_eq!(wowpay_watch_receive_actions(&watch, Chain::Ethereum, &[], BsvHomeRuntimeEnvironment::WowPay), None);
        assert_eq!(
            wowpay_watch_receive_actions(&wallet(WalletType::View, &[Chain::Ethereum]), Chain::BitcoinSv, &[], BsvHomeRuntimeEnvironment::WowPay),
            None
        );
        assert_eq!(
            wowpay_watch_receive_actions(&wallet(WalletType::Single, &[Chain::BitcoinSv]), Chain::BitcoinSv, &[], BsvHomeRuntimeEnvironment::WowPay),
            None
        );
    }

    #[test]
    fn test_default_asset_basic_uses_default_rank_and_properties() {
        let native = default_asset_basic(Asset::from_chain(Chain::Ethereum));
        let token = default_asset_basic(Asset::new(
            AssetId::from(Chain::Ethereum, Some("0x0000000000000000000000000000000000000001".to_string())),
            String::new(),
            String::new(),
            18,
            primitives::AssetType::ERC20,
        ));

        assert_eq!(native.score.rank, Chain::Ethereum.rank());
        assert!(native.score.rank > token.score.rank);
        assert!(native.properties.is_enabled);
        assert!(native.price.is_none());
    }

    #[test]
    fn test_default_balances_by_wallet_type() {
        let (enabled, disabled) = default_balances(&wallet(WalletType::Multicoin, &[Chain::Cosmos, Chain::Ethereum, Chain::BitcoinSv, Chain::Tron]));
        assert!(disabled.contains(&AssetId::from_chain(Chain::Cosmos)));
        assert!(enabled.contains(&AssetId::from_chain(Chain::Ethereum)));
        assert!(enabled.contains(&AssetId::from_chain(Chain::BitcoinSv)));
        assert!(wallet_default_assets(Chain::Tron).iter().all(|asset| enabled.contains(&asset.id)));

        let (enabled, disabled) = default_balances(&wallet(WalletType::Single, &[Chain::Cosmos]));
        assert_eq!(enabled, vec![AssetId::from_chain(Chain::Cosmos)]);
        assert!(disabled.is_empty());

        let (enabled, _) = default_balances(&wallet(WalletType::Single, &[Chain::Tempo]));
        assert!(!enabled.contains(&AssetId::from_chain(Chain::Tempo)));
        assert!(wallet_default_assets(Chain::Tempo).iter().all(|asset| enabled.contains(&asset.id)));
    }

    #[test]
    fn test_missing_asset_ids_drops_known_and_duplicate_ids() {
        let bitcoin = AssetId::from_chain(Chain::Bitcoin);
        let ethereum = AssetId::from_chain(Chain::Ethereum);

        let missing = missing_asset_ids(vec![bitcoin.clone(), ethereum.clone(), ethereum.clone()], vec![bitcoin]);

        assert_eq!(missing, vec![ethereum]);
    }

    #[test]
    fn test_asset_list_is_outdated_only_when_the_stored_version_differs() {
        assert!(is_asset_list_outdated(None, 7));
        assert!(is_asset_list_outdated(Some("6"), 7));
        assert!(!is_asset_list_outdated(Some("7"), 7));
    }

    #[test]
    fn test_asset_ids_skips_unparsable_identifiers() {
        let ids = asset_ids(&["bitcoin".to_string(), String::new(), "ethereum_0x1234".to_string()]);

        assert_eq!(ids, vec![AssetId::from_chain(Chain::Bitcoin), AssetId::from_token(Chain::Ethereum, "0x1234")]);
    }

    #[test]
    fn test_swappable_chain_asset_ids_only_lists_swap_supported_chains() {
        let asset_ids = swappable_chain_asset_ids();

        assert!(asset_ids.contains(&AssetId::from_chain(Chain::Ethereum)));
        assert!(asset_ids.iter().all(|asset_id| asset_id.chain.is_swap_supported()));
    }

    #[test]
    fn test_token_search_chains_defaults_to_every_chain() {
        assert_eq!(token_search_chains(&[Chain::Ethereum]), vec![Chain::Ethereum]);
        assert_eq!(token_search_chains(&[]), Chain::all());
    }

    fn metadata(is_balance_enabled: bool, is_buy_enabled: bool, is_swap_enabled: bool, is_earn_enabled: bool) -> AssetMetaData {
        AssetMetaData {
            is_enabled: true,
            is_balance_enabled,
            is_buy_enabled,
            is_sell_enabled: false,
            is_swap_enabled,
            is_stake_enabled: false,
            is_earn_enabled,
            is_pinned: false,
            is_active: true,
            staking_apr: None,
            earn_apr: None,
            rank_score: 0,
        }
    }

    fn state(wallet_type: WalletType, chain: Chain, metadata: &AssetMetaData, banner_events: &[BannerEvent]) -> BsvHomeAssetDetailsState {
        details_state(wallet_type, chain, metadata, &BsvHomeAssetBalance::mock(), banner_events, Some(1.0), vec![])
    }

    fn buttons(state: &BsvHomeAssetDetailsState) -> Vec<BsvHomeHeaderButton> {
        match &state.header_actions {
            BsvHomeHeaderActions::Buttons { buttons } => buttons.clone(),
            BsvHomeHeaderActions::WatchOnly => panic!("the header offers no buttons"),
        }
    }

    fn kinds(state: &BsvHomeAssetDetailsState) -> Vec<BsvHomeHeaderButtonKind> {
        buttons(state).into_iter().map(|button| button.kind).collect()
    }

    #[test]
    fn test_testnet_details_allow_only_transfers_despite_retained_market_metadata_and_prices() {
        for asset in [Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()] {
            let asset_id = asset.id;
            let state = details_state(
                WalletType::Multicoin,
                asset_id.chain,
                &metadata(true, true, true, true),
                &BsvHomeAssetBalance::zero(asset_id.clone()),
                &[],
                Some(1.0),
                vec![PriceAlert::new_auto(asset_id, Currency::USD)],
            );
            assert_eq!(
                state,
                BsvHomeAssetDetailsState {
                    is_view_only: false,
                    header_actions: BsvHomeHeaderActions::Buttons {
                        buttons: vec![
                            BsvHomeHeaderButton {
                                kind: BsvHomeHeaderButtonKind::Send,
                                is_enabled: true
                            },
                            BsvHomeHeaderButton {
                                kind: BsvHomeHeaderButtonKind::Receive,
                                is_enabled: true
                            },
                        ],
                    },
                    shows_banners: false,
                    shows_manage: false,
                    shows_resources: false,
                    shows_price_alerts: false,
                    shows_price_change: false,
                    shows_earn: false,
                    empty_transactions_action: None,
                }
            );
        }
    }

    #[test]
    fn test_details_state_offers_portal_swap_only_for_the_exact_supported_assets() {
        let cases = [
            (Chain::BitcoinSv.as_asset_id(), WalletType::Multicoin),
            (primitives::asset_constants::TRON_USDT_ASSET_ID.clone(), WalletType::Multicoin),
            (AssetId::from_token(Chain::Tron, "unrelated-contract"), WalletType::Multicoin),
            (Chain::Tron.as_asset_id(), WalletType::Multicoin),
            (Chain::Ethereum.as_asset_id(), WalletType::Multicoin),
            (Chain::BitcoinSv.as_asset_id(), WalletType::View),
        ];
        let offered: Vec<_> = cases
            .into_iter()
            .map(|(asset_id, wallet_type)| {
                let state = details_state(
                    wallet_type,
                    asset_id.chain,
                    &metadata(true, false, false, false),
                    &BsvHomeAssetBalance::zero(asset_id),
                    &[],
                    None,
                    vec![],
                );
                match state.header_actions {
                    BsvHomeHeaderActions::WatchOnly => false,
                    BsvHomeHeaderActions::Buttons { buttons } => buttons.iter().any(|button| button.kind == BsvHomeHeaderButtonKind::Swap && button.is_enabled),
                }
            })
            .collect();
        assert_eq!(offered, vec![true, true, false, false, false, false]);
    }

    #[test]
    fn test_details_state_shows_the_header_buttons_the_metadata_allows() {
        let all = state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, true, true, false), &[]);
        assert_eq!(
            kinds(&all),
            vec![
                BsvHomeHeaderButtonKind::Send,
                BsvHomeHeaderButtonKind::Receive,
                BsvHomeHeaderButtonKind::Buy,
                BsvHomeHeaderButtonKind::Swap
            ]
        );
        assert!(buttons(&all).iter().all(|button| button.is_enabled));

        let transfer_only = state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, false, false, false), &[]);
        assert_eq!(kinds(&transfer_only), vec![BsvHomeHeaderButtonKind::Send, BsvHomeHeaderButtonKind::Receive]);
    }

    #[test]
    fn test_details_state_disables_price_change_only_for_current_price_assets() {
        let asset_ids = [
            Chain::BitcoinSv.as_asset_id(),
            primitives::asset_constants::TRON_USDT_ASSET_ID.clone(),
            Chain::Tron.as_asset_id(),
            Chain::Ethereum.as_asset_id(),
            AssetId::from_token(Chain::Tron, "other-token"),
        ];
        let supported: Vec<_> = asset_ids
            .into_iter()
            .map(|asset_id| {
                let balance = BsvHomeAssetBalance::zero(asset_id.clone());
                details_state(
                    WalletType::Multicoin,
                    asset_id.chain,
                    &metadata(true, false, false, false),
                    &balance,
                    &[],
                    Some(1.0),
                    vec![],
                )
                .shows_price_change
            })
            .collect();
        assert_eq!(supported, vec![false, false, true, true, true]);
    }

    #[test]
    fn test_details_state_disables_the_buttons_behind_an_activation_or_multi_signature_banner() {
        for event in [BannerEvent::ActivateAsset, BannerEvent::AccountBlockedMultiSignature] {
            let state = state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, true, true, false), &[BannerEvent::Stake, event]);
            assert!(buttons(&state).iter().all(|button| !button.is_enabled), "{event:?}");
        }
        let stake_only = state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, true, true, false), &[BannerEvent::Stake]);
        assert!(buttons(&stake_only).iter().all(|button| button.is_enabled));
    }

    #[test]
    fn test_details_state_hides_buttons_banners_and_earn_from_a_view_only_wallet() {
        let state = state(WalletType::View, Chain::Ethereum, &metadata(true, true, true, true), &[]);

        assert!(state.is_view_only);
        assert_eq!(state.header_actions, BsvHomeHeaderActions::WatchOnly);
        assert!(!state.shows_banners);
        assert!(!state.shows_earn);
        assert_eq!(state.empty_transactions_action, Some(BsvHomeAssetEmptyAction::Buy));
    }

    #[test]
    fn test_details_state_shows_banners_for_every_wallet_type() {
        for wallet_type in [WalletType::Multicoin, WalletType::Single, WalletType::PrivateKey, WalletType::View] {
            for event in [BannerEvent::AccountBlockedMultiSignature, BannerEvent::SuspiciousAsset] {
                let state = state(wallet_type, Chain::Tron, &metadata(true, true, true, true), &[event]);
                assert!(state.shows_banners, "{wallet_type:?} {event:?}");
            }
            assert!(!state(wallet_type, Chain::Tron, &metadata(true, true, true, true), &[]).shows_banners);
        }
    }

    #[test]
    fn test_details_state_offers_manage_until_the_balance_is_enabled() {
        assert!(state(WalletType::Multicoin, Chain::Ethereum, &metadata(false, false, false, false), &[]).shows_manage);
        assert!(!state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, false, false, false), &[]).shows_manage);

        let disabled_asset = AssetMetaData {
            is_enabled: false,
            ..metadata(false, false, false, false)
        };
        assert!(state(WalletType::Multicoin, Chain::Ethereum, &disabled_asset, &[]).shows_manage);
    }

    #[test]
    fn test_trx_details_never_offer_balance_management() {
        let states: Vec<_> = [Chain::Tron.as_asset_id(), Chain::TronShasta.as_asset_id(), TRON_SHASTA_USDT.id.clone()]
            .into_iter()
            .map(|asset_id| {
                details_state(
                    WalletType::Multicoin,
                    asset_id.chain,
                    &metadata(false, false, false, false),
                    &BsvHomeAssetBalance::zero(asset_id),
                    &[],
                    None,
                    vec![],
                )
                .shows_manage
            })
            .collect();
        assert_eq!(states, vec![false, true, true]);
    }

    #[test]
    fn test_trx_details_exclude_swap_even_when_metadata_enables_it() {
        let offered: Vec<_> = [Chain::Tron.as_asset_id(), wallet_default_assets(Chain::Tron)[0].id.clone(), Chain::Ethereum.as_asset_id()]
            .into_iter()
            .map(|asset_id| {
                let state = details_state(
                    WalletType::Multicoin,
                    asset_id.chain,
                    &metadata(true, false, true, false),
                    &BsvHomeAssetBalance::zero(asset_id),
                    &[],
                    None,
                    vec![],
                );
                (kinds(&state), state.empty_transactions_action)
            })
            .collect();
        let transfer_actions = vec![BsvHomeHeaderButtonKind::Send, BsvHomeHeaderButtonKind::Receive];
        let swap_actions = vec![BsvHomeHeaderButtonKind::Send, BsvHomeHeaderButtonKind::Receive, BsvHomeHeaderButtonKind::Swap];
        assert_eq!(
            offered,
            vec![
                (transfer_actions, None),
                (swap_actions.clone(), Some(BsvHomeAssetEmptyAction::Swap)),
                (swap_actions, Some(BsvHomeAssetEmptyAction::Swap))
            ]
        );
    }

    #[test]
    fn test_details_state_shows_resources_only_where_staking_freezes() {
        assert!(state(WalletType::Multicoin, Chain::Tron, &metadata(true, false, false, false), &[]).shows_resources);
        assert!(!state(WalletType::Multicoin, Chain::Cosmos, &metadata(true, false, false, false), &[]).shows_resources);
        assert!(!state(WalletType::Multicoin, Chain::Bitcoin, &metadata(true, false, false, false), &[]).shows_resources);
    }

    #[test]
    fn test_details_state_shows_price_alerts_only_with_alerts_and_a_price() {
        let plain = metadata(true, false, false, false);
        let balance = BsvHomeAssetBalance::mock();

        let auto = PriceAlert::new_auto(AssetId::from_chain(Chain::Ethereum), Currency::USD);
        let mut notified = PriceAlert::new_price(AssetId::from_chain(Chain::Ethereum), Currency::USD, 120.0, PriceAlertDirection::Up);
        notified.last_notified_at = Some(Utc::now());
        let shows = |price: Option<f64>, alerts: Vec<PriceAlert>| details_state(WalletType::Multicoin, Chain::Ethereum, &plain, &balance, &[], price, alerts).shows_price_alerts;

        assert!(shows(Some(1.0), vec![auto.clone(), notified.clone()]));
        assert!(!shows(Some(1.0), vec![notified]));
        assert!(!shows(Some(0.0), vec![auto.clone()]));
        assert!(!shows(None, vec![auto]));
        assert!(!shows(Some(1.0), vec![]));
    }

    #[test]
    fn test_details_state_offers_earn_until_there_is_an_earn_balance() {
        let earn_enabled = metadata(true, false, false, true);

        assert!(details_state(WalletType::Multicoin, Chain::Ethereum, &earn_enabled, &BsvHomeAssetBalance::mock(), &[], Some(1.0), vec![]).shows_earn);
        let earning = BsvHomeAssetBalance {
            earn: BsvHomeBigUint::from(100u32),
            ..BsvHomeAssetBalance::mock()
        };
        assert!(!details_state(WalletType::Multicoin, Chain::Ethereum, &earn_enabled, &earning, &[], Some(1.0), vec![]).shows_earn);
        assert!(
            !details_state(
                WalletType::Multicoin,
                Chain::Ethereum,
                &metadata(true, false, false, false),
                &BsvHomeAssetBalance::mock(),
                &[],
                Some(1.0),
                vec![]
            )
            .shows_earn
        );
    }

    #[test]
    fn test_details_state_empty_transactions_prefer_buy_then_swap() {
        assert_eq!(
            state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, true, true, false), &[]).empty_transactions_action,
            Some(BsvHomeAssetEmptyAction::Buy)
        );
        assert_eq!(
            state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, false, true, false), &[]).empty_transactions_action,
            Some(BsvHomeAssetEmptyAction::Swap)
        );
        assert_eq!(
            state(WalletType::Multicoin, Chain::Ethereum, &metadata(true, false, false, false), &[]).empty_transactions_action,
            None
        );
    }
}
