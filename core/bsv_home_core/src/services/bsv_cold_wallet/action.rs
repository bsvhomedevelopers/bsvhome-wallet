use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use bsv_home_hash::sha2::sha256;
use primitives::{Wallet, WalletId};
use uuid::Uuid;

use super::{BsvHomeBsvColdWalletStateStamp, BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState, BsvHomeOfflineSigningPasswordDomain};
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::BsvHomeWalletService;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvColdWalletActionIntent {
    Prepare { recipient: String, amount: u64 },
    Sign { commitment: String },
    Broadcast { commitment: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvColdWalletSubmission {
    NotSubmitted,
    Unknown,
    Submitted,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvColdWalletActionOutcome {
    pub submission: BsvHomeBsvColdWalletSubmission,
    pub transaction_id: Option<String>,
    pub requires_reload: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Running,
    Finished,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PasswordGrant {
    Missing,
    Granted,
    Consumed,
}

#[derive(Clone)]
struct SigningPayload {
    request_hash: String,
    review_hash: String,
}

#[derive(Clone)]
struct Binding {
    wallet: Wallet,
    stamp: Option<BsvHomeBsvColdWalletStateStamp>,
    signing: Option<SigningPayload>,
}

struct ActionState {
    phase: Phase,
    binding: Option<Binding>,
    password: PasswordGrant,
    outcome: BsvHomeBsvColdWalletActionOutcome,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvColdWalletActionScope {
    id: Uuid,
    owner: Uuid,
    epoch: Arc<AtomicU64>,
    generation: u64,
    wallet_id: WalletId,
    wallets: Arc<BsvHomeWalletService>,
    store: Arc<dyn BsvHomeBsvColdWalletStore>,
    domain: BsvHomeOfflineSigningPasswordDomain,
    intent: BsvHomeBsvColdWalletActionIntent,
    cancelled: AtomicBool,
    state: Mutex<ActionState>,
}

#[uniffi::export]
impl BsvHomeBsvColdWalletActionScope {
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn check(&self) -> Result<(), BsvHomeServiceError> {
        self.check_liveness()?;
        let binding = self.binding()?;
        if self.store.state_stamp(self.wallet_id.clone())? != binding.stamp {
            self.cancel();
            return Err(BsvHomeServiceError::Cancelled);
        }
        self.check_liveness()
    }

    pub async fn validate(&self) -> Result<(), BsvHomeServiceError> {
        self.check()?;
        let binding = self.binding()?;
        let wallet = self.wallets.wallets().await?.into_iter().find(|wallet| wallet.id == self.wallet_id);
        self.check()?;
        if wallet
            .as_ref()
            .is_none_or(|wallet| wallet.wallet_type != binding.wallet.wallet_type || wallet.accounts != binding.wallet.accounts)
        {
            self.cancel();
            return Err(BsvHomeServiceError::Cancelled);
        }
        let record = self.store.get(self.wallet_id.clone()).await?;
        self.check()?;
        if record.as_ref().map(BsvHomeBsvColdWalletStoredState::stamp) != binding.stamp {
            self.cancel();
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }

    pub fn check_signing(&self, wallet: Wallet, request_json: String, review_json: String) -> Result<(), BsvHomeServiceError> {
        self.check()?;
        match &self.intent {
            BsvHomeBsvColdWalletActionIntent::Sign { .. } => {}
            BsvHomeBsvColdWalletActionIntent::Prepare { .. } | BsvHomeBsvColdWalletActionIntent::Broadcast { .. } => return Err(BsvHomeServiceError::Cancelled),
        }
        let binding = self.binding()?;
        let signing = binding.signing.ok_or(BsvHomeServiceError::Cancelled)?;
        if serde_json::to_vec(&wallet).map_err(|_| BsvHomeServiceError::Cancelled)? != serde_json::to_vec(&binding.wallet).map_err(|_| BsvHomeServiceError::Cancelled)?
            || hex::encode(sha256(request_json.as_bytes())) != signing.request_hash
            || hex::encode(sha256(review_json.as_bytes())) != signing.review_hash
        {
            return Err(BsvHomeServiceError::Cancelled);
        }
        if self.state()?.password != PasswordGrant::Consumed {
            return Err(password_required());
        }
        Ok(())
    }

    pub fn outcome(&self) -> Result<BsvHomeBsvColdWalletActionOutcome, BsvHomeServiceError> {
        Ok(self.state()?.outcome.clone())
    }
}

impl BsvHomeBsvColdWalletActionScope {
    pub(super) fn new(
        owner: Uuid,
        epoch: Arc<AtomicU64>,
        wallet_id: WalletId,
        wallets: Arc<BsvHomeWalletService>,
        store: Arc<dyn BsvHomeBsvColdWalletStore>,
        domain: BsvHomeOfflineSigningPasswordDomain,
        intent: BsvHomeBsvColdWalletActionIntent,
    ) -> Arc<Self> {
        let generation = epoch.load(Ordering::SeqCst);
        Arc::new(Self {
            id: Uuid::new_v4(),
            owner,
            epoch,
            generation,
            wallet_id,
            wallets,
            store,
            domain,
            intent,
            cancelled: AtomicBool::new(false),
            state: Mutex::new(ActionState {
                phase: Phase::Ready,
                binding: None,
                password: PasswordGrant::Missing,
                outcome: BsvHomeBsvColdWalletActionOutcome {
                    submission: BsvHomeBsvColdWalletSubmission::NotSubmitted,
                    transaction_id: None,
                    requires_reload: false,
                },
            }),
        })
    }

    pub(super) fn check_liveness(&self) -> Result<(), BsvHomeServiceError> {
        if self.generation == u64::MAX
            || self.cancelled.load(Ordering::SeqCst)
            || self.epoch.load(Ordering::SeqCst) != self.generation
            || self.wallets.current_wallet_id()? != Some(self.wallet_id.clone())
            || self.state()?.phase == Phase::Finished
        {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }

    pub(super) fn claim(self: &Arc<Self>, owner: Uuid, wallet_id: &WalletId, intent: &BsvHomeBsvColdWalletActionIntent) -> Result<ActionClaim, BsvHomeServiceError> {
        self.check_liveness()?;
        let mut state = self.state()?;
        if owner != self.owner || wallet_id != &self.wallet_id || intent != &self.intent || state.phase != Phase::Ready {
            return Err(BsvHomeServiceError::Cancelled);
        }
        state.phase = Phase::Running;
        Ok(ActionClaim { scope: self.clone() })
    }

    pub(super) fn bind(&self, wallet: Wallet, record: Option<&BsvHomeBsvColdWalletStoredState>, signing: Option<(&str, &str)>) -> Result<(), BsvHomeServiceError> {
        self.check_liveness()?;
        if wallet.id != self.wallet_id {
            return Err(BsvHomeServiceError::Cancelled);
        }
        {
            let mut state = self.state()?;
            if state.phase != Phase::Running || state.binding.is_some() {
                return Err(BsvHomeServiceError::Cancelled);
            }
            state.binding = Some(Binding {
                wallet,
                stamp: record.map(BsvHomeBsvColdWalletStoredState::stamp),
                signing: signing.map(|(request, review)| SigningPayload {
                    request_hash: hex::encode(sha256(request.as_bytes())),
                    review_hash: hex::encode(sha256(review.as_bytes())),
                }),
            });
        }
        self.check()
    }

    pub(crate) fn password_wallet(&self) -> Result<Wallet, BsvHomeServiceError> {
        self.check()?;
        match &self.intent {
            BsvHomeBsvColdWalletActionIntent::Sign { .. } => Ok(self.binding()?.wallet),
            BsvHomeBsvColdWalletActionIntent::Prepare { .. } | BsvHomeBsvColdWalletActionIntent::Broadcast { .. } => Err(BsvHomeServiceError::Cancelled),
        }
    }

    pub(crate) fn password_domain(&self) -> BsvHomeOfflineSigningPasswordDomain {
        self.domain
    }

    pub(crate) fn grant_password(&self) -> Result<(), BsvHomeServiceError> {
        self.password_wallet()?;
        let mut state = self.state()?;
        if state.phase != Phase::Running || state.password != PasswordGrant::Missing {
            return Err(BsvHomeServiceError::Cancelled);
        }
        state.password = PasswordGrant::Granted;
        Ok(())
    }

    pub(super) fn consume_password(&self) -> Result<(), BsvHomeServiceError> {
        self.check()?;
        let mut state = self.state()?;
        if state.password != PasswordGrant::Granted {
            return Err(password_required());
        }
        state.password = PasswordGrant::Consumed;
        Ok(())
    }

    pub(super) fn begin_write(&self) -> Result<(), BsvHomeServiceError> {
        self.check()?;
        self.write_pending()
    }

    pub(super) fn write_pending(&self) -> Result<(), BsvHomeServiceError> {
        self.state()?.outcome.requires_reload = true;
        Ok(())
    }

    pub(super) fn write_committed(&self) -> Result<(), BsvHomeServiceError> {
        self.state()?.outcome.requires_reload = false;
        Ok(())
    }

    pub(super) fn begin_submission(&self) -> Result<(), BsvHomeServiceError> {
        self.check()?;
        self.state()?.outcome.submission = BsvHomeBsvColdWalletSubmission::Unknown;
        Ok(())
    }

    pub(super) fn submitted(&self, transaction_id: String) -> Result<(), BsvHomeServiceError> {
        let mut state = self.state()?;
        state.outcome.submission = BsvHomeBsvColdWalletSubmission::Submitted;
        state.outcome.transaction_id = Some(transaction_id);
        Ok(())
    }

    fn binding(&self) -> Result<Binding, BsvHomeServiceError> {
        let state = self.state()?;
        if state.phase != Phase::Running {
            return Err(BsvHomeServiceError::Cancelled);
        }
        state.binding.clone().ok_or(BsvHomeServiceError::Cancelled)
    }

    fn state(&self) -> Result<MutexGuard<'_, ActionState>, BsvHomeServiceError> {
        self.state.lock().map_err(|_| BsvHomeServiceError::Store {
            msg: "Cold action state could not be read".into(),
        })
    }
}

pub(super) struct ActionClaim {
    scope: Arc<BsvHomeBsvColdWalletActionScope>,
}

impl Drop for ActionClaim {
    fn drop(&mut self) {
        if let Ok(mut state) = self.scope.state.lock() {
            state.phase = Phase::Finished;
        }
    }
}

fn password_required() -> BsvHomeServiceError {
    BsvHomeServiceError::InvalidInput {
        msg: "Offline signing password authorization is required".into(),
    }
}
