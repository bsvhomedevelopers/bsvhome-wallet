use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bsv_home_bsv::BsvAddress;
use bsv_home_bsv::multisig::{Funding, FundingReview, Policy, Proposal, SignatureSet, member_address, policy_descriptor, sign_funding_with_keys, sign_proposal};
use bsv_home_hash::sha2::sha256;
use chrono::Utc;
use futures::channel::oneshot;
use primitives::{Account, Address, Chain, Wallet, WalletId, WalletType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use signer::secp256k1_public_key;
use tempfile::TempDir;
use zeroize::Zeroizing;

use super::model::{RemoteMember, RemoteProposal, RemoteWallet, State};
use super::{
    BsvHomeBsvMultisigActionIntent, BsvHomeBsvMultisigActionScope, BsvHomeBsvMultisigError, BsvHomeBsvMultisigService, BsvHomeBsvMultisigSigner, BsvHomeBsvMultisigStore,
    BsvHomeBsvMultisigStoredState,
};
use crate::alien::{AlienError, AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::services::BsvHomeServiceError;
use crate::services::bsv_cold_wallet::testkit::MemoryColdWalletStore;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::portal::BsvHomePortalAuthService;
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet::addresses::testkit::pool_with_keystore;
use crate::services::wallet::testkit::{MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;

pub(crate) const TEST_BASE_URL: &str = "https://api-dev.hj0252.com/api/portal";
pub(crate) const TEST_V2_BASE_URL: &str = "https://api-dev.hj0252.com/v2/api/portal";
pub(crate) const TEST_WALLET_ID: &str = "team-wallet";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InteropFixture {
    pub proposal: Proposal,
    pub signature_sets: Vec<SignatureSet>,
    pub signed_transaction: String,
    pub signed_transaction_id: String,
}

pub(crate) fn fixture() -> InteropFixture {
    serde_json::from_str(include_str!("../../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap()
}

pub(crate) fn member(index: u8, confirmed: bool) -> RemoteMember {
    let public_key = hex::encode(secp256k1_public_key(&[index; 32]).unwrap());
    RemoteMember {
        user_id: i64::from(index),
        address: member_address(&public_key).unwrap(),
        public_key,
        confirmed,
    }
}

pub(crate) fn remote_wallet(status: &str, confirmed: bool) -> RemoteWallet {
    let descriptor = policy_descriptor(&fixture().proposal.policy).unwrap();
    RemoteWallet {
        id: TEST_WALLET_ID.into(),
        name: "Team wallet".into(),
        status: status.into(),
        threshold: 2,
        member_count: 3,
        version: 7,
        members: (1..=3).map(|index| member(index, confirmed)).collect(),
        locking_script: Some(descriptor.locking_script),
        script_hash: Some(descriptor.script_hash),
        configuration_hash: Some(descriptor.configuration_hash),
    }
}

pub(crate) fn remote_proposal(id: &str, status: &str) -> RemoteProposal {
    let now = Utc::now().timestamp();
    let fixture = fixture();
    RemoteProposal {
        id: id.into(),
        wallet_id: TEST_WALLET_ID.into(),
        status: status.into(),
        creator_id: Some(1),
        creator_address: Some(member(1, true).address),
        note: Some("Team payment".into()),
        version: 3,
        created_at: now - 60,
        expires_at: now + 86400,
        txid: if matches!(status, "BROADCAST" | "CONFIRMED") {
            Some(fixture.signed_transaction_id)
        } else {
            None
        },
        confirmations: Some(0),
        proposal: fixture.proposal,
        signatures: if matches!(status, "READY" | "BROADCAST" | "CONFIRMED") {
            fixture.signature_sets.into_iter().take(2).collect()
        } else {
            Vec::new()
        },
    }
}

#[derive(Default)]
pub(crate) struct MemoryBsvMultisigStore {
    pub state: Mutex<Option<BsvHomeBsvMultisigStoredState>>,
    pub fail_next_cas: Mutex<bool>,
    pub fail_write: Mutex<bool>,
    pub on_commit: Mutex<Option<Hook>>,
}

impl MemoryBsvMultisigStore {
    pub fn read(&self) -> State {
        self.state
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| serde_json::from_str(&state.json).unwrap())
            .unwrap_or_default()
    }
}

#[async_trait]
impl BsvHomeBsvMultisigStore for MemoryBsvMultisigStore {
    fn get_state(&self) -> Result<Option<BsvHomeBsvMultisigStoredState>, BsvHomeServiceError> {
        Ok(self.state.lock().unwrap().clone())
    }
    async fn compare_and_swap(&self, expected_revision: u64, json: String) -> Result<bool, BsvHomeServiceError> {
        if *self.fail_write.lock().unwrap() {
            return Err(BsvHomeServiceError::Store {
                msg: "multisig test storage unavailable".into(),
            });
        }
        if std::mem::take(&mut *self.fail_next_cas.lock().unwrap()) {
            return Ok(false);
        }
        let mut state = self.state.lock().unwrap();
        if state.as_ref().map_or(0, |value| value.revision) != expected_revision {
            return Ok(false);
        }
        *state = Some(BsvHomeBsvMultisigStoredState {
            revision: expected_revision + 1,
            json,
        });
        drop(state);
        let hook = self.on_commit.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(true)
    }
}

type Hook = Box<dyn Fn() + Send + Sync>;
type RequestHook = Arc<dyn Fn(&AlienTarget) + Send + Sync>;
type Pause = (oneshot::Sender<()>, oneshot::Receiver<()>);

pub(crate) struct MemoryBsvMultisigSigner {
    pub index: u8,
    pub authentications: Mutex<u32>,
    pub proposal_signatures: Mutex<u32>,
    pub funding_signatures: Mutex<u32>,
    pub cancelled: Mutex<bool>,
    pub on_authenticate: Mutex<Option<Hook>>,
    pub on_sign: Mutex<Option<Hook>>,
    pub sign_pause: Mutex<Option<Pause>>,
}

impl MemoryBsvMultisigSigner {
    pub fn new(index: u8) -> Self {
        Self {
            index,
            authentications: Mutex::new(0),
            proposal_signatures: Mutex::new(0),
            funding_signatures: Mutex::new(0),
            cancelled: Mutex::new(false),
            on_authenticate: Mutex::new(None),
            on_sign: Mutex::new(None),
            sign_pause: Mutex::new(None),
        }
    }
    async fn signing_checkpoint(&self) -> Result<(), BsvHomeServiceError> {
        let pause = self.sign_pause.lock().unwrap().take();
        if let Some((started, release)) = pause {
            started.send(()).unwrap();
            release.await.unwrap();
        }
        if let Some(action) = self.on_sign.lock().unwrap().take() {
            action();
        }
        if *self.cancelled.lock().unwrap() {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }
}

#[async_trait]
impl BsvHomeBsvMultisigSigner for MemoryBsvMultisigSigner {
    async fn public_key(&self, _: Wallet) -> Result<String, BsvHomeServiceError> {
        if *self.cancelled.lock().unwrap() {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(member(self.index, false).public_key)
    }
    async fn authenticate(&self, _: Wallet, scope: Option<Arc<BsvHomeBsvMultisigActionScope>>) -> Result<(), BsvHomeServiceError> {
        if let Some(scope) = &scope {
            scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        *self.authentications.lock().unwrap() += 1;
        if let Some(action) = self.on_authenticate.lock().unwrap().take() {
            action();
        }
        if *self.cancelled.lock().unwrap() {
            return Err(BsvHomeServiceError::Cancelled);
        }
        if let Some(scope) = &scope {
            scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        Ok(())
    }
    async fn sign_proposal(
        &self,
        _: Wallet,
        policy_json: String,
        proposal_json: String,
        recipient: String,
        amount: u64,
        maximum_fee: u64,
        scope: Arc<BsvHomeBsvMultisigActionScope>,
    ) -> Result<String, BsvHomeServiceError> {
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        self.signing_checkpoint().await?;
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        *self.proposal_signatures.lock().unwrap() += 1;
        let policy: Policy = serde_json::from_str(&policy_json).unwrap();
        let proposal: Proposal = serde_json::from_str(&proposal_json).unwrap();
        let signatures = sign_proposal(&policy, &proposal, &recipient, amount, maximum_fee, &[self.index; 32]).unwrap();
        Ok(serde_json::to_string(&signatures).unwrap())
    }
    async fn sign_funding(&self, wallet: Wallet, funding_json: String, review_json: String, scope: Arc<BsvHomeBsvMultisigActionScope>) -> Result<String, BsvHomeServiceError> {
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        self.signing_checkpoint().await?;
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        *self.funding_signatures.lock().unwrap() += 1;
        let funding: Funding = serde_json::from_str(&funding_json).unwrap();
        let review: FundingReview = serde_json::from_str(&review_json).unwrap();
        let address = &wallet.account(Chain::BitcoinSv).unwrap().address;
        let index = (1..=3).find(|index| member(*index, true).address == *address).unwrap();
        Ok(sign_funding_with_keys(&funding, &review, |_| Ok(Zeroizing::new(vec![index; 32]))).unwrap())
    }
}

pub(crate) struct KeystoreMultisigSigner {
    keystore: Arc<BsvHomeKeystore>,
    pub signed_wallets: Mutex<Vec<WalletId>>,
}

#[async_trait]
impl BsvHomeBsvMultisigSigner for KeystoreMultisigSigner {
    async fn public_key(&self, wallet: Wallet) -> Result<String, BsvHomeServiceError> {
        Ok(self
            .keystore
            .bsv_multisig_public_key(self.keystore.keystore_id(wallet.id.id()), decode_password(TEST_PASSWORD))?)
    }

    async fn authenticate(&self, wallet: Wallet, scope: Option<Arc<BsvHomeBsvMultisigActionScope>>) -> Result<(), BsvHomeServiceError> {
        if let Some(scope) = &scope {
            scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        self.public_key(wallet).await?;
        if let Some(scope) = &scope {
            scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        Ok(())
    }

    async fn sign_proposal(
        &self,
        wallet: Wallet,
        policy_json: String,
        proposal_json: String,
        recipient: String,
        amount: u64,
        maximum_fee: u64,
        scope: Arc<BsvHomeBsvMultisigActionScope>,
    ) -> Result<String, BsvHomeServiceError> {
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        let signature = self.keystore.bsv_multisig_sign(
            self.keystore.keystore_id(wallet.id.id()),
            policy_json,
            proposal_json,
            recipient,
            amount,
            maximum_fee,
            decode_password(TEST_PASSWORD),
        )?;
        self.signed_wallets.lock().unwrap().push(wallet.id);
        Ok(signature)
    }

    async fn sign_funding(&self, wallet: Wallet, funding_json: String, review_json: String, scope: Arc<BsvHomeBsvMultisigActionScope>) -> Result<String, BsvHomeServiceError> {
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        Ok(self
            .keystore
            .bsv_multisig_funding_sign(self.keystore.keystore_id(wallet.id.id()), funding_json, review_json, decode_password(TEST_PASSWORD))?)
    }
}

pub(crate) struct KeystoreMultisigDevice {
    pub service: BsvHomeBsvMultisigService,
    pub cold_store: Arc<MemoryColdWalletStore>,
    pub signer: Arc<KeystoreMultisigSigner>,
    pub wallet: Wallet,
    pub provider: Arc<MemoryMultisigProvider>,
    _directory: TempDir,
}

impl KeystoreMultisigDevice {
    pub fn new(index: u8, provider: Arc<MemoryMultisigProvider>) -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::PrivateKey {
                    value: hex::encode([index; 32]),
                    chain: Chain::BitcoinSv,
                },
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let wallet = Device::wallet(index);
        assert_eq!(stored.wallet_id, wallet.id.id());
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let session = Arc::new(BsvHomeWalletSessionService::new(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet.id.clone())),
            }),
            wallets.clone(),
        ));
        let password = Arc::new(MemoryKeystorePassword::default());
        let secure = Arc::new(MemorySecureStore::default());
        secure
            .set(
                "session".into(),
                serde_json::json!({
                    "base_url": provider.base_url,
                    "wallet_id": wallet.id.id(),
                    "address": member(index, true).address,
                    "token": format!("public-multisig-member-{index}-test-token"),
                    "expires_at": null,
                })
                .to_string(),
            )
            .unwrap();
        let auth = Arc::new(
            BsvHomePortalAuthService::new(
                provider.clone(),
                keystore.clone(),
                password.clone(),
                session.clone(),
                Arc::new(BsvHomeDeviceKeyService::new(Arc::new(MemorySecureStore::default()))),
                secure,
                provider.base_url.clone(),
                "zh-CN".into(),
                "4".into(),
            )
            .unwrap(),
        );
        let gateway = Arc::new(BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences)));
        let addresses = pool_with_keystore(gateway.clone(), wallets.clone(), keystore.clone(), password, wallets.bsv_addresses.clone());
        let signer = Arc::new(KeystoreMultisigSigner {
            keystore,
            signed_wallets: Mutex::new(Vec::new()),
        });
        let cold_store = Arc::new(MemoryColdWalletStore::default());
        let service = BsvHomeBsvMultisigService::new(
            Arc::new(MemoryBsvMultisigStore::default()),
            session,
            gateway,
            signer.clone(),
            auth,
            addresses,
            wallet.id.id(),
            cold_store.clone(),
        );
        Self {
            service,
            cold_store,
            signer,
            wallet,
            provider,
            _directory: directory,
        }
    }

    pub fn reopened_service(&self) -> BsvHomeBsvMultisigService {
        BsvHomeBsvMultisigService::new(
            self.service.store.clone(),
            self.service.session.clone(),
            self.service.gateway.clone(),
            self.signer.clone(),
            self.service.auth.clone(),
            self.service.addresses.clone(),
            self.wallet.id.id(),
            self.cold_store.clone(),
        )
    }
}

#[derive(Default)]
struct MemorySecureStore {
    values: Mutex<BTreeMap<String, String>>,
}
impl BsvHomeSecureStore for MemorySecureStore {
    fn get(&self, key: String) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(self.values.lock().unwrap().get(&key).cloned())
    }
    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().insert(key, value);
        Ok(())
    }
    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().remove(&key);
        Ok(())
    }
}

#[derive(Serialize)]
struct Envelope<T> {
    success: bool,
    code: &'static str,
    data: T,
}
fn response<T: Serialize>(data: T) -> Arc<AlienResponse> {
    Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&Envelope { success: true, code: "OK", data }).unwrap()))
}
fn rejected(code: &str) -> Arc<AlienResponse> {
    #[derive(Serialize)]
    struct Rejection<'a> {
        code: &'a str,
        message: &'a str,
    }
    Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&Rejection { code, message: code }).unwrap()))
}

pub(crate) struct MemoryMultisigProvider {
    pub wallets: Arc<Mutex<Vec<RemoteWallet>>>,
    pub proposals: Arc<Mutex<Vec<RemoteProposal>>>,
    pub requests: Mutex<Vec<AlienTarget>>,
    pub duplicate_second_page: Mutex<bool>,
    pub fail_prepare_after_commit: Mutex<bool>,
    pub fail_create_after_commit: Mutex<bool>,
    pub on_create: Mutex<Option<Hook>>,
    pub on_request: Mutex<Option<RequestHook>>,
    pub fail_balances: Mutex<bool>,
    pub overrides: Mutex<HashMap<String, Arc<AlienResponse>>>,
    pub node_responses: Mutex<HashMap<String, Arc<AlienResponse>>>,
    pub broadcasts: Arc<Mutex<Vec<String>>>,
    pub fail_broadcast: Mutex<bool>,
    pub on_accepted_funding: Mutex<Option<Hook>>,
    pub invitation_generation: Arc<Mutex<u8>>,
    invited_wallet_id: Arc<Mutex<String>>,
    accepted_funding: Mutex<Vec<String>>,
    prepared: Arc<Mutex<HashMap<String, RemoteProposal>>>,
    identity_index: u8,
    base_url: String,
}
impl Debug for MemoryMultisigProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("MemoryMultisigProvider")
    }
}
impl MemoryMultisigProvider {
    pub fn new(identity_index: u8, base_url: &str) -> Self {
        Self {
            identity_index,
            base_url: base_url.into(),
            wallets: Arc::new(Mutex::new(vec![remote_wallet("ACTIVE", true)])),
            proposals: Arc::new(Mutex::new(Vec::new())),
            requests: Mutex::new(Vec::new()),
            duplicate_second_page: Mutex::new(false),
            fail_prepare_after_commit: Mutex::new(false),
            fail_create_after_commit: Mutex::new(false),
            on_create: Mutex::new(None),
            on_request: Mutex::new(None),
            fail_balances: Mutex::new(false),
            overrides: Mutex::new(HashMap::new()),
            node_responses: Mutex::new(HashMap::new()),
            broadcasts: Arc::new(Mutex::new(Vec::new())),
            fail_broadcast: Mutex::new(false),
            on_accepted_funding: Mutex::new(None),
            invitation_generation: Arc::new(Mutex::new(0)),
            invited_wallet_id: Arc::new(Mutex::new("invited-wallet".into())),
            accepted_funding: Mutex::new(Vec::new()),
            prepared: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn for_member(&self, index: u8) -> Self {
        Self {
            wallets: self.wallets.clone(),
            proposals: self.proposals.clone(),
            broadcasts: self.broadcasts.clone(),
            invitation_generation: self.invitation_generation.clone(),
            invited_wallet_id: self.invited_wallet_id.clone(),
            prepared: self.prepared.clone(),
            ..Self::new(index, &self.base_url)
        }
    }
    pub fn paths(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|target| target.url.strip_prefix(&self.base_url).unwrap_or(&target.url).to_string())
            .collect()
    }
    pub fn posts(&self) -> Vec<AlienTarget> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|target| target.method == AlienHttpMethod::Post)
            .cloned()
            .collect()
    }
    pub fn override_data<T: Serialize>(&self, path: &str, data: T) {
        self.overrides.lock().unwrap().insert(path.into(), response(data));
    }

    pub fn override_post_data<T: Serialize>(&self, path: &str, data: T) {
        self.overrides.lock().unwrap().insert(format!("POST {path}"), response(data));
    }

    pub fn override_error(&self, path: &str, code: &str) {
        self.overrides.lock().unwrap().insert(path.into(), rejected(code));
    }

    pub fn invitation_token(&self) -> String {
        format!("{}A", char::from(b'A' + *self.invitation_generation.lock().unwrap()).to_string().repeat(42))
    }

    pub fn seed_funding(&self, index: u8) {
        #[derive(Serialize)]
        struct Output {
            tx_hash: String,
            tx_pos: u32,
            value: u64,
            height: u64,
            status: &'static str,
            #[serde(rename = "isSpentInMempoolTx")]
            spent: bool,
        }
        #[derive(Serialize)]
        struct Outputs<'a> {
            address: &'a str,
            result: Vec<Output>,
            error: &'static str,
        }
        let address = member(index, true).address;
        let transaction_id = "11".repeat(32);
        let data = Outputs {
            address: &address,
            result: vec![Output {
                tx_hash: transaction_id.clone(),
                tx_pos: 0,
                value: 200000,
                height: 100,
                status: "confirmed",
                spent: false,
            }],
            error: "",
        };
        self.node_responses.lock().unwrap().insert(
            format!("/address/{address}/unspent/all"),
            Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&data).unwrap())),
        );
        let mut transaction: Value = serde_json::from_str(include_str!("../../../../crates/bsv_home_bsv/testdata/wallet_spent_activity.json")).unwrap();
        transaction["txid"] = transaction_id.clone().into();
        transaction["vout"][0]["value"] = Value::Number("0.002".parse().unwrap());
        transaction["vout"][0]["scriptPubKey"]["hex"] = hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()).into();
        self.node_responses.lock().unwrap().insert(
            format!("/tx/hash/{transaction_id}"),
            Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&transaction).unwrap())),
        );
        self.node_responses.lock().unwrap().insert(
            "/feerecommendation".into(),
            Arc::new(AlienResponse::new(Some(200), br#"{"fee_unit":"sat/KB","fee":100,"mempool_min_fee":100}"#.to_vec())),
        );
    }
    fn balance(&self) -> Arc<AlienResponse> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Utxo {
            txid: String,
            output_index: u32,
            satoshis: u64,
            transaction_hex: String,
        }
        let mut items = fixture()
            .proposal
            .inputs
            .into_iter()
            .zip([30_000, 70_000])
            .map(|(input, satoshis)| Utxo {
                txid: transaction_id(&input.transaction_hex),
                output_index: input.output_index,
                satoshis,
                transaction_hex: input.transaction_hex,
            })
            .collect::<Vec<_>>();
        for raw in self.accepted_funding.lock().unwrap().iter() {
            items.push(Utxo {
                txid: transaction_id(raw),
                output_index: 0,
                satoshis: 100000,
                transaction_hex: raw.clone(),
            });
        }
        response(items)
    }
}
#[async_trait]
impl AlienProvider for MemoryMultisigProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.requests.lock().unwrap().push(target.clone());
        let hook = self.on_request.lock().unwrap().clone();
        if let Some(hook) = hook {
            hook(&target);
        }
        let path = target.url.strip_prefix(&self.base_url).unwrap_or(&target.url);
        if target.method == AlienHttpMethod::Post
            && let Some(response) = self.overrides.lock().unwrap().get(&format!("POST {path}")).cloned()
        {
            return Ok(response);
        }
        if let Some(response) = self.overrides.lock().unwrap().get(path).cloned() {
            return Ok(response);
        }
        let body: Value = target.body.as_ref().map(|bytes| serde_json::from_slice(bytes).unwrap()).unwrap_or(Value::Null);
        if path == "/auth/multisig/wallets" {
            if target.method == AlienHttpMethod::Get {
                let identity = member(self.identity_index, true).address;
                return Ok(response(
                    self.wallets
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|wallet| wallet.members.iter().any(|member| member.address == identity))
                        .cloned()
                        .collect::<Vec<_>>(),
                ));
            }
            let created = RemoteWallet {
                id: format!("created-wallet-{}", self.wallets.lock().unwrap().len()),
                name: body["name"].as_str().unwrap().into(),
                status: "DRAFT".into(),
                threshold: body["threshold"].as_u64().unwrap() as u32,
                member_count: body["memberCount"].as_u64().unwrap() as u32,
                version: 0,
                members: vec![member(self.identity_index, false)],
                locking_script: None,
                configuration_hash: None,
                script_hash: None,
            };
            self.wallets.lock().unwrap().push(created.clone());
            if let Some(hook) = self.on_create.lock().unwrap().take() {
                hook();
            }
            if *self.fail_create_after_commit.lock().unwrap() {
                return Err(AlienError::RequestError {
                    msg: "temporary create response unavailable".into(),
                });
            }
            return Ok(response(created));
        }
        if path == "/auth/multisig/invites/preview" {
            let token = self.invitation_token();
            if body["token"].as_str() != Some(token.as_str()) {
                return Ok(rejected("INVALID_INVITE"));
            }
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Preview {
                wallet: RemoteWallet,
                expires_at: i64,
            }
            let invited_wallet_id = self.invited_wallet_id.lock().unwrap().clone();
            let wallet = self.wallets.lock().unwrap().iter().find(|wallet| wallet.id == invited_wallet_id).unwrap().clone();
            return Ok(response(Preview {
                wallet,
                expires_at: Utc::now().timestamp() + 86400,
            }));
        }
        if path == "/auth/multisig/join" {
            let token = self.invitation_token();
            if body["token"].as_str() != Some(token.as_str()) {
                return Ok(rejected("INVALID_INVITE"));
            }
            let mut wallets = self.wallets.lock().unwrap();
            let wallet = wallets.iter_mut().find(|wallet| wallet.id == body["walletId"].as_str().unwrap()).unwrap();
            if body["expectedVersion"].as_i64() != Some(wallet.version) {
                return Ok(rejected("STALE_VERSION"));
            }
            wallet.members.push(member(self.identity_index, false));
            wallet.version += 1;
            if wallet.members.len() == wallet.member_count as usize {
                let descriptor = policy_descriptor(&Policy {
                    threshold: wallet.threshold,
                    public_keys: wallet.members.iter().map(|member| member.public_key.clone()).collect(),
                })
                .unwrap();
                wallet.locking_script = Some(descriptor.locking_script);
                wallet.script_hash = Some(descriptor.script_hash);
                wallet.configuration_hash = Some(descriptor.configuration_hash);
            }
            return Ok(response(wallet.clone()));
        }
        if path.starts_with("/auth/multisig/wallets/") {
            let components = path.trim_start_matches("/auth/multisig/wallets/").split('/').collect::<Vec<_>>();
            let wallet = self.wallets.lock().unwrap().iter().find(|wallet| wallet.id == components[0]).unwrap().clone();
            let action = components.get(1).copied().unwrap_or("");
            if action.is_empty() || action == "config" {
                return Ok(response(wallet));
            }
            if action.starts_with("proposal-page?") {
                #[derive(Serialize)]
                #[serde(rename_all = "camelCase")]
                struct Page {
                    items: Vec<RemoteProposal>,
                    total: usize,
                    offset: usize,
                    limit: usize,
                    has_more: bool,
                }
                let offset = action.split("offset=").nth(1).unwrap().split('&').next().unwrap().parse::<usize>().unwrap();
                let proposals = self
                    .proposals
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|proposal| proposal.wallet_id == wallet.id)
                    .cloned()
                    .collect::<Vec<_>>();
                let start = if offset > 0 && *self.duplicate_second_page.lock().unwrap() {
                    offset - 1
                } else {
                    offset
                };
                let items = proposals.iter().skip(start).take(200).cloned().collect::<Vec<_>>();
                return Ok(response(Page {
                    items,
                    total: proposals.len(),
                    offset,
                    limit: 200,
                    has_more: offset + 200 < proposals.len(),
                }));
            }
            if action == "utxos" {
                if *self.fail_balances.lock().unwrap() {
                    return Err(AlienError::Offline);
                }
                return Ok(self.balance());
            }
            if body["expectedVersion"].as_i64() != Some(wallet.version) {
                return Ok(rejected("STALE_VERSION"));
            }
            if action == "confirm" {
                let mut wallets = self.wallets.lock().unwrap();
                let current = wallets.iter_mut().find(|item| item.id == wallet.id).unwrap();
                current
                    .members
                    .iter_mut()
                    .find(|member| member.user_id == i64::from(self.identity_index))
                    .unwrap()
                    .confirmed = true;
                if current.members.iter().all(|member| member.confirmed) {
                    current.status = "ACTIVE".into();
                }
                return Ok(response(current.clone()));
            }
            if action == "invites" {
                #[derive(Serialize)]
                #[serde(rename_all = "camelCase")]
                struct Invite {
                    token: String,
                    expires_at: i64,
                    wallet_id: String,
                }
                *self.invitation_generation.lock().unwrap() += 1;
                *self.invited_wallet_id.lock().unwrap() = wallet.id.clone();
                self.wallets.lock().unwrap().iter_mut().find(|item| item.id == wallet.id).unwrap().version += 1;
                return Ok(response(Invite {
                    token: self.invitation_token(),
                    expires_at: Utc::now().timestamp() + 86400,
                    wallet_id: wallet.id,
                }));
            }
            if action == "proposals" {
                let key = body["idempotencyKey"].as_str().unwrap().to_string();
                let mut prepared = self.prepared.lock().unwrap();
                let proposal = prepared
                    .entry(key)
                    .or_insert_with(|| {
                        let mut proposal = remote_proposal("prepared-proposal", "COLLECTING");
                        proposal.wallet_id = wallet.id.clone();
                        proposal.note = Some(body["note"].as_str().unwrap().into());
                        self.proposals.lock().unwrap().push(proposal.clone());
                        proposal
                    })
                    .clone();
                if std::mem::take(&mut *self.fail_prepare_after_commit.lock().unwrap()) {
                    return Err(AlienError::Offline);
                }
                return Ok(response(proposal));
            }
        }
        if let Some(route) = path.strip_prefix("/auth/multisig/proposals/") {
            let (id, action) = route.split_once('/').unwrap();
            let mut proposals = self.proposals.lock().unwrap();
            let current = proposals.iter_mut().find(|proposal| proposal.id == id).unwrap();
            if action != "refresh" && body["expectedVersion"].as_i64() != Some(current.version) {
                return Ok(rejected("STALE_VERSION"));
            }
            match action {
                "signatures" => {
                    current.signatures.push(SignatureSet {
                        public_key: body["publicKey"].as_str().unwrap().into(),
                        signatures: serde_json::from_value(body["signatures"].clone()).unwrap(),
                    });
                    if current.signatures.len() >= current.proposal.policy.threshold as usize {
                        current.status = "READY".into();
                    }
                }
                "withdraw" => current.status = "WITHDRAWN".into(),
                "broadcast" => {
                    let raw = body["rawTransaction"].as_str().unwrap();
                    self.broadcasts.lock().unwrap().push(raw.into());
                    current.txid = Some(transaction_id(raw));
                    current.status = "BROADCAST".into();
                }
                "refresh" => {
                    if current.status == "CONFIRMED" {
                        current.status = "BROADCAST".into();
                        current.confirmations = Some(0);
                    }
                }
                _ => panic!("unexpected multisig proposal operation"),
            }
            current.version += 1;
            return Ok(response(current.clone()));
        }
        let node_path = target.url.strip_prefix("https://api.whatsonchain.com/v1/bsv/main").unwrap_or(path);
        if node_path == "/tx/raw" {
            let raw = body["txhex"].as_str().unwrap().to_string();
            self.broadcasts.lock().unwrap().push(raw.clone());
            if *self.fail_broadcast.lock().unwrap() {
                return Ok(Arc::new(AlienResponse::new(Some(200), b"unexpected-transaction-id".to_vec())));
            }
            {
                let mut accepted = self.accepted_funding.lock().unwrap();
                if !accepted.contains(&raw) {
                    accepted.push(raw.clone());
                }
            }
            let hook = self.on_accepted_funding.lock().unwrap().take();
            if let Some(hook) = hook {
                hook();
            }
            return Ok(Arc::new(AlienResponse::new(Some(200), transaction_id(&raw).into_bytes())));
        }
        if let Some(response) = self.node_responses.lock().unwrap().get(node_path).cloned() {
            return Ok(response);
        }
        Ok(Arc::new(AlienResponse::new(Some(404), Vec::new())))
    }
}
pub(crate) fn transaction_id(raw: &str) -> String {
    let digest = sha256(&sha256(&hex::decode(raw).unwrap()));
    hex::encode(digest.into_iter().rev().collect::<Vec<_>>())
}

pub(crate) struct Device {
    pub service: BsvHomeBsvMultisigService,
    pub cold_store: Arc<MemoryColdWalletStore>,
    pub store: Arc<MemoryBsvMultisigStore>,
    pub signer: Arc<MemoryBsvMultisigSigner>,
    pub provider: Arc<MemoryMultisigProvider>,
    pub wallet: Wallet,
    pub wallets: Arc<MemoryWalletStore>,
    pub session: Arc<BsvHomeWalletSessionService>,
    pub gateway: Arc<BsvHomeGateway>,
    pub addresses: Arc<BsvHomeBsvAddressPool>,
    pub auth: Arc<BsvHomePortalAuthService>,
    _directory: TempDir,
}
impl Device {
    pub fn reopened_service(&self) -> BsvHomeBsvMultisigService {
        BsvHomeBsvMultisigService::new(
            self.store.clone(),
            self.session.clone(),
            self.gateway.clone(),
            self.signer.clone(),
            self.auth.clone(),
            self.addresses.clone(),
            self.wallet.id.id(),
            self.cold_store.clone(),
        )
    }

    pub fn new(index: u8) -> Self {
        Self::with_origin(index, TEST_BASE_URL)
    }
    pub fn with_origin(index: u8, base_url: &str) -> Self {
        Self::with_scope(index, base_url, None)
    }
    pub fn with_fixed_wallet(index: u8) -> Self {
        Self::with_scope(index, TEST_BASE_URL, Some(Self::wallet(index)))
    }
    fn with_scope(index: u8, base_url: &str, fixed_wallet: Option<Wallet>) -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let password = Arc::new(MemoryKeystorePassword::default());
        let wallet = Self::wallet(index);
        let wallets = Arc::new(MemoryWalletStore::default());
        *wallets.wallets.lock().unwrap() = (1..=3).map(Self::wallet).collect();
        let session = Arc::new(BsvHomeWalletSessionService::new(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet.id.clone())),
            }),
            wallets.clone(),
        ));
        let provider = Arc::new(MemoryMultisigProvider::new(index, base_url));
        let secure = Arc::new(MemorySecureStore::default());
        let mut cached: Value =
            serde_json::from_str(r#"{"base_url":"https://api-dev.hj0252.com/api/portal","wallet_id":"","address":"","token":"public-multisig-test-token","expires_at":null}"#)
                .unwrap();
        cached["wallet_id"] = wallet.id.id().into();
        cached["address"] = member(index, true).address.into();
        cached["base_url"] = base_url.into();
        secure.set("session".into(), serde_json::to_string(&cached).unwrap()).unwrap();
        let device_key = Arc::new(BsvHomeDeviceKeyService::new(Arc::new(MemorySecureStore::default())));
        let auth = Arc::new(match fixed_wallet {
            Some(fixed_wallet) => BsvHomePortalAuthService::new_for_wallet(
                provider.clone(),
                keystore.clone(),
                password.clone(),
                session.clone(),
                device_key,
                secure,
                base_url.into(),
                "zh-CN".into(),
                "4".into(),
                fixed_wallet,
            )
            .unwrap(),
            None => BsvHomePortalAuthService::new(
                provider.clone(),
                keystore.clone(),
                password.clone(),
                session.clone(),
                device_key,
                secure,
                base_url.into(),
                "zh-CN".into(),
                "4".into(),
            )
            .unwrap(),
        });
        let gateway = Arc::new(BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences)));
        let addresses = pool_with_keystore(gateway.clone(), wallets.clone(), keystore, password, wallets.bsv_addresses.clone());
        let store = Arc::new(MemoryBsvMultisigStore::default());
        let signer = Arc::new(MemoryBsvMultisigSigner::new(index));
        let cold_store = Arc::new(MemoryColdWalletStore::default());
        let service = BsvHomeBsvMultisigService::new(
            store.clone(),
            session.clone(),
            gateway.clone(),
            signer.clone(),
            auth.clone(),
            addresses.clone(),
            wallet.id.id(),
            cold_store.clone(),
        );
        Self {
            service,
            cold_store,
            store,
            signer,
            provider,
            wallet,
            wallets,
            session,
            gateway,
            addresses,
            auth,
            _directory: directory,
        }
    }
    pub fn wallet(index: u8) -> Wallet {
        let identity = member(index, true);
        Wallet {
            id: WalletId::PrivateKey(Chain::BitcoinSv, identity.address.clone()),
            wallet_type: WalletType::PrivateKey,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, &identity.address)])
        }
    }
    pub async fn trust(&self) {
        let (revision, mut state) = self.service.read().unwrap();
        state.trusted.insert(self.service.scope(TEST_WALLET_ID), fixture().proposal.policy);
        self.service.save(revision, state).await.unwrap();
    }
}

#[async_trait]
pub(crate) trait ScopedTestActions {
    async fn scoped_prepare_proposal(
        &self,
        wallet_id: String,
        destination: String,
        amount_bsv: String,
        memo: String,
    ) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError>;
    async fn scoped_sign_proposal(&self, proposal_id: String, expected_version: u64) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError>;
    async fn scoped_broadcast_proposal(&self, proposal_id: String, expected_version: u64) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError>;
    async fn scoped_withdraw_proposal(&self, proposal_id: String, expected_version: u64) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError>;
    async fn scoped_submit_funding(&self, preparation_id: String) -> Result<super::BsvHomeBsvMultisigFundingResult, BsvHomeBsvMultisigError>;
    async fn scoped_retry_funding(&self, wallet_id: String) -> Result<super::BsvHomeBsvMultisigFundingResult, BsvHomeBsvMultisigError>;
    async fn scoped_update_paymail_settings(
        &self,
        settings: super::BsvHomeBsvMultisigPaymailSettings,
        alias: String,
        active: bool,
    ) -> Result<super::BsvHomeBsvMultisigPaymailSettings, BsvHomeBsvMultisigError>;
}

#[async_trait]
impl ScopedTestActions for BsvHomeBsvMultisigService {
    async fn scoped_prepare_proposal(
        &self,
        wallet_id: String,
        destination: String,
        amount_bsv: String,
        memo: String,
    ) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::PrepareProposal {
            wallet_id: wallet_id.clone(),
            destination: destination.clone(),
            amount_bsv: amount_bsv.clone(),
            memo: memo.clone(),
        })?;
        self.prepare_proposal(wallet_id, destination, amount_bsv, memo, action).await
    }
    async fn scoped_sign_proposal(&self, proposal_id: String, expected_version: u64) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::SignProposal {
            proposal_id: proposal_id.clone(),
            expected_version,
            review_id: self.proposal_review_snapshot(proposal_id.clone())?.review_id,
        })?;
        self.sign_proposal(proposal_id, expected_version, action).await
    }
    async fn scoped_broadcast_proposal(&self, proposal_id: String, expected_version: u64) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::BroadcastProposal {
            proposal_id: proposal_id.clone(),
            expected_version,
            review_id: self.proposal_review_snapshot(proposal_id.clone())?.review_id,
        })?;
        self.broadcast_proposal(proposal_id, expected_version, action).await
    }
    async fn scoped_withdraw_proposal(&self, proposal_id: String, expected_version: u64) -> Result<super::BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::WithdrawProposal {
            proposal_id: proposal_id.clone(),
            expected_version,
            review_id: self.proposal_review_snapshot(proposal_id.clone())?.review_id,
        })?;
        self.withdraw_proposal(proposal_id, expected_version, action).await
    }
    async fn scoped_submit_funding(&self, preparation_id: String) -> Result<super::BsvHomeBsvMultisigFundingResult, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding {
            preparation_id: preparation_id.clone(),
        })?;
        self.submit_funding(preparation_id, action).await
    }
    async fn scoped_retry_funding(&self, wallet_id: String) -> Result<super::BsvHomeBsvMultisigFundingResult, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::RetryFunding { wallet_id: wallet_id.clone() })?;
        self.retry_funding(wallet_id, action).await
    }
    async fn scoped_update_paymail_settings(
        &self,
        settings: super::BsvHomeBsvMultisigPaymailSettings,
        alias: String,
        active: bool,
    ) -> Result<super::BsvHomeBsvMultisigPaymailSettings, BsvHomeBsvMultisigError> {
        self.current_wallet().await?;
        let action = self.begin_action(BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings {
            wallet_id: settings.wallet_id.clone(),
            alias: alias.clone(),
            active,
            expected_version: settings.version,
            configuration_hash: settings.configuration_hash.clone(),
        })?;
        self.update_paymail_settings(settings, alias, active, action).await
    }
}
