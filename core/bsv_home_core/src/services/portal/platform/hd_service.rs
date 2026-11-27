use std::sync::Arc;

use futures::try_join;
use primitives::{Account, BsvAddressBranch, Chain, Wallet, WalletId, WalletType};

use crate::keystore::{BsvHomeKeystore, decode_password, keystore_id_for_wallet};
use crate::message::sign_type::{SignDigestType, SignMessage};
use crate::message::signer::{MessageSigner, recover_bitcoin_message_address};
use crate::services::wallet::BsvHomeKeystorePassword;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet_session::BsvHomeWalletSessionService;

use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountService, BsvHomePlatformHdAccount, BsvHomePlatformHdAddress, BsvHomePlatformHdLocalWallet, BsvHomePlatformHdOverview,
    BsvHomePlatformHdScan, BsvHomePlatformHdSnapshot,
};

#[derive(uniffi::Object)]
pub struct BsvHomePlatformHdService {
    platform: Arc<BsvHomePlatformAccountService>,
    session: Arc<BsvHomeWalletSessionService>,
    addresses: Arc<BsvHomeBsvAddressPool>,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
}

#[uniffi::export]
impl BsvHomePlatformHdService {
    #[uniffi::constructor]
    pub fn new(
        platform: Arc<BsvHomePlatformAccountService>,
        session: Arc<BsvHomeWalletSessionService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        keystore: Arc<BsvHomeKeystore>,
        password: Arc<dyn BsvHomeKeystorePassword>,
    ) -> Self {
        Self {
            platform,
            session,
            addresses,
            keystore,
            password,
        }
    }

    pub async fn overview(&self) -> Result<BsvHomePlatformHdOverview, BsvHomePlatformAccountError> {
        let lease = self.platform.session_lease()?;
        let (local_wallets, accounts) = try_join!(self.local_wallets(), self.accounts())?;
        self.platform.require_lease(&lease)?;
        Ok(BsvHomePlatformHdOverview { local_wallets, accounts })
    }

    pub async fn local_wallets(&self) -> Result<Vec<BsvHomePlatformHdLocalWallet>, BsvHomePlatformAccountError> {
        self.session
            .get_wallets()
            .await?
            .iter()
            .filter_map(|wallet| match local_wallet(wallet) {
                Ok(local) => Some(Ok(local)),
                Err(BsvHomePlatformAccountError::InvalidInput) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    pub async fn accounts(&self) -> Result<Vec<BsvHomePlatformHdAccount>, BsvHomePlatformAccountError> {
        self.platform.hd_accounts().await
    }

    pub async fn enroll(&self, local_wallet_id: String, label: String, consent: bool) -> Result<BsvHomePlatformHdAccount, BsvHomePlatformAccountError> {
        if !consent {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let wallet_id = WalletId::from_id(&local_wallet_id).ok_or(BsvHomePlatformAccountError::InvalidInput)?;
        let active = self.session.get_current_wallet_id()?;
        let lease = self.platform.session_lease()?;
        let wallet = self.wallet(wallet_id.clone()).await?;
        let account = self.addresses.cold_account(wallet_id.clone()).await?;
        self.require_wallet(&wallet, &active).await?;
        self.platform.require_lease(&lease)?;
        let challenge = self.platform.hd_enrollment_challenge(account.clone(), label, true).await?;
        let signer = MessageSigner::new(SignMessage {
            chain: Chain::BitcoinSv,
            sign_type: SignDigestType::BitcoinMessage,
            data: challenge.message.as_bytes().to_vec(),
        });
        let password = decode_password(&self.password.get_password(false)?);
        self.require_wallet(&wallet, &active).await?;
        self.platform.require_lease(&lease)?;
        let signature = signer
            .sign_with_keystore(self.keystore.clone(), keystore_id_for_wallet(local_wallet_id), password)
            .map_err(|_| BsvHomePlatformAccountError::InvalidInput)?;
        if recover_bitcoin_message_address(challenge.message.as_bytes(), &signature).map_err(|_| BsvHomePlatformAccountError::InvalidInput)? != account.address {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        self.require_wallet(&wallet, &active).await?;
        self.platform.require_lease(&lease)?;
        let enrolled = self.platform.hd_enroll(challenge.id, signature).await?;
        self.require_wallet(&wallet, &active).await?;
        self.platform.require_lease(&lease)?;
        Ok(enrolled)
    }

    pub async fn snapshot(&self, id: String, local_wallet_id: String) -> Result<BsvHomePlatformHdSnapshot, BsvHomePlatformAccountError> {
        let active = self.session.get_current_wallet_id()?;
        let (wallet, account, indexes) = self.local_account(local_wallet_id).await?;
        self.require_wallet(&wallet, &active).await?;
        let snapshot = self.platform.hd_snapshot(id, account, indexes).await?;
        self.require_wallet(&wallet, &active).await?;
        Ok(snapshot)
    }

    pub async fn allocate(&self, id: String, local_wallet_id: String) -> Result<BsvHomePlatformHdAddress, BsvHomePlatformAccountError> {
        let active = self.session.get_current_wallet_id()?;
        let (wallet, account, indexes) = self.local_account(local_wallet_id).await?;
        self.require_wallet(&wallet, &active).await?;
        let address = self.platform.hd_allocate(id, account, indexes).await?;
        self.require_wallet(&wallet, &active).await?;
        Ok(address)
    }

    pub async fn start_scan(&self, id: String, gap_limit: u32, max_index: u32) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        self.platform.hd_start_scan(id, gap_limit, max_index).await
    }

    pub async fn scan_status(&self, id: String) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        self.platform.hd_scan_status(id).await
    }
    pub async fn cancel_scan(&self, id: String) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        self.platform.hd_cancel_scan(id).await
    }
    pub async fn resume_scan(&self, id: String, max_index: u32) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        self.platform.hd_resume_scan(id, max_index).await
    }
    pub fn cancel_workflow(&self) -> Result<(), BsvHomePlatformAccountError> {
        self.platform.cancel()
    }
}

impl BsvHomePlatformHdService {
    async fn wallet(&self, id: WalletId) -> Result<Wallet, BsvHomePlatformAccountError> {
        let wallet = self.session.get_wallet(id).await?.ok_or(BsvHomePlatformAccountError::InvalidInput)?;
        local_wallet(&wallet)?;
        Ok(wallet)
    }

    async fn local_account(&self, id: String) -> Result<(Wallet, Account, Vec<u32>), BsvHomePlatformAccountError> {
        let id = WalletId::from_id(&id).ok_or(BsvHomePlatformAccountError::InvalidInput)?;
        let wallet = self.wallet(id.clone()).await?;
        let account = self.addresses.cold_account(id.clone()).await?;
        let snapshot = self.addresses.snapshot(id).await?.ok_or(BsvHomePlatformAccountError::InvalidInput)?;
        let indexes = snapshot
            .records
            .iter()
            .filter(|record| record.branch == BsvAddressBranch::Receive)
            .map(|record| record.index)
            .collect();
        Ok((wallet, account, indexes))
    }

    async fn require_wallet(&self, original: &Wallet, active: &Option<WalletId>) -> Result<(), BsvHomePlatformAccountError> {
        if self.session.get_current_wallet_id()? != *active || self.wallet(original.id.clone()).await?.accounts != original.accounts {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        Ok(())
    }
}

fn local_wallet(wallet: &Wallet) -> Result<BsvHomePlatformHdLocalWallet, BsvHomePlatformAccountError> {
    match wallet.wallet_type {
        WalletType::Multicoin | WalletType::Single => {}
        WalletType::PrivateKey | WalletType::View => return Err(BsvHomePlatformAccountError::InvalidInput),
    }
    let account = wallet.account(Chain::BitcoinSv).ok_or(BsvHomePlatformAccountError::InvalidInput)?;
    let path = account.derivation_path.strip_suffix("/0/0").ok_or(BsvHomePlatformAccountError::InvalidInput)?;
    Ok(BsvHomePlatformHdLocalWallet {
        id: wallet.id.id(),
        name: wallet.name.clone(),
        first_address: account.address.clone(),
        base_path: path.into(),
    })
}
