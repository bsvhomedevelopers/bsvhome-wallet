use primitives::Asset;

use crate::models::BsvHomeStakeChain;

#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum DocsUrl {
    Start,
    WhatIsWatchWallet,
    WhatIsSecretPhrase,
    WhatIsPrivateKey,
    HowToSecureSecretPhrase,
    TransactionStatus,
    NetworkFees,
    StakingLockTime,
    TronMultiSignature,
    RootedDevice,
    PriceImpact,
    TokenApproval,
    Slippage,
    SwapProvider,
    FiatProvider,
    StakingAPR,
    StakingStatus,
    StakingValidator,
    AccountMinimalBalance,
    TokenVerification,
    AddCustomToken,
    WalletConnect,
    HowStoreSecretPhrase,
    NoQuotes,
    Staking(BsvHomeStakeChain),
    PerpetualsFundingRate,
    PerpetualsLiquidationPrice,
    PerpetualsOpenInterest,
    PerpetualsFundingPayments,
    PerpetualsAutoclose,
    Dust,
    MigrateWallet,
}
pub(super) const DOCS_URL: &str = "https://api-dev.hj0252.com/help";

#[uniffi::export]
impl DocsUrl {
    pub fn url(&self) -> String {
        let path = match self {
            Self::Start => "/",
            Self::WhatIsWatchWallet => "/faq/watch-wallet/",
            Self::WhatIsSecretPhrase => "/faq/secret-recovery-phrase/",
            Self::WhatIsPrivateKey => "/faq/private-key/",
            Self::HowToSecureSecretPhrase => "/faq/secure-recovery-phrase/",
            Self::TransactionStatus => "/faq/transaction-status/",
            Self::NetworkFees => "/faq/network-fees/",
            Self::StakingLockTime => "/faq/lock-time/",
            Self::TronMultiSignature => "/guides/trx-multisig-scam/",
            Self::RootedDevice => "/guides/secure-wallet/rooted-device/",
            Self::PriceImpact => "/faq/price-impact/",
            Self::TokenApproval => "/faq/token-approval/",
            Self::Slippage => "/faq/slippage/",
            Self::SwapProvider => "/faq/swap-provider/",
            Self::FiatProvider => "/faq/fiat-provider/",
            Self::StakingAPR => "/faq/staking-apr/",
            Self::StakingStatus => "/faq/staking-status/",
            Self::StakingValidator => "/faq/staking-validator/",
            Self::AccountMinimalBalance => "/faq/account-minimal-balance/",
            Self::TokenVerification => "/faq/token-verification/",
            Self::AddCustomToken => "/guides/add-token/",
            Self::WalletConnect => "/guides/walletconnect/",
            Self::HowStoreSecretPhrase => "/faq/secure-recovery-phrase/",
            Self::NoQuotes => "/troubleshoot/quote-error/",
            Self::Staking(chain) => &format!("/defi/stake-{}/", Asset::from_chain(chain.chain()).symbol.to_lowercase()),
            Self::PerpetualsFundingRate => "/defi/perps/funding-apr/",
            Self::PerpetualsLiquidationPrice => "/defi/perps/liquidation-price/",
            Self::PerpetualsOpenInterest => "/defi/perps/open-interest/",
            Self::PerpetualsFundingPayments => "/defi/perps/funding-payment/",
            Self::PerpetualsAutoclose => "/defi/perps/auto-close/",
            Self::Dust => "/blockchains/bitcoin/dust/",
            Self::MigrateWallet => "/guides/migrate-wallet/",
        };
        format!("{DOCS_URL}{path}")
    }
}

#[cfg(test)]
mod tests {
    use primitives::StakeChain;

    use super::*;

    #[test]
    fn test_get_docs_url() {
        assert_eq!(DocsUrl::WhatIsSecretPhrase.url(), "https://api-dev.hj0252.com/help/faq/secret-recovery-phrase/");
    }

    #[test]
    fn test_get_docs_url_staking() {
        assert_eq!(DocsUrl::Staking(StakeChain::Solana).url(), "https://api-dev.hj0252.com/help/defi/stake-sol/");
    }
}
