use std::collections::HashMap;

use bsv_home_bsv::{BsvAddress, cold_wallet::Pairing};
use bsv_home_hash::sha2::sha256;
use primitives::{Account, Address};
use serde_json::{Value, json};

use crate::message::signer::recover_bitcoin_message_address;

use super::rules;
use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountService, BsvHomePlatformHdAccount, BsvHomePlatformHdAddress, BsvHomePlatformHdBalance, BsvHomePlatformHdEnrollment,
    BsvHomePlatformHdScan, BsvHomePlatformHdScanAction, BsvHomePlatformHdSnapshot,
};

impl BsvHomePlatformAccountService {
    pub async fn hd_accounts(&self) -> Result<Vec<BsvHomePlatformHdAccount>, BsvHomePlatformAccountError> {
        let values = self.authenticated_json("portal/hd/accounts", json!({})).await?;
        values
            .as_array()
            .ok_or(BsvHomePlatformAccountError::InvalidResponse)?
            .iter()
            .cloned()
            .map(hd_account)
            .collect()
    }

    pub async fn hd_enrollment_challenge(&self, account: Account, label: String, consent: bool) -> Result<BsvHomePlatformHdEnrollment, BsvHomePlatformAccountError> {
        if !consent {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        rules::label(&label)?;
        let pairing = Pairing::from_account(&account).map_err(|_| BsvHomePlatformAccountError::InvalidInput)?;
        let lease = self.session_lease()?;
        let response = self
            .authenticated_json(
                "portal/hd/enroll/challenge",
                json!({ "accountXpub": pairing.xpub, "basePath": pairing.account_path, "label": label }),
            )
            .await?;
        let id = rules::string(&response, "id")?;
        rules::identifier(id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
        let message = rules::string(&response, "message")?;
        let expires = rules::string(&response, "expiresAt")?;
        rules::unexpired(expires)?;
        let action_hash = rules::string(&response, "actionHash")?;
        let fields = enrollment_fields(message)?;
        if fields.get("User") != Some(&lease.user_id.as_str())
            || fields.get("Account XPUB") != Some(&pairing.xpub.as_str())
            || fields.get("Base Path") != Some(&pairing.account_path.as_str())
            || fields.get("Address") != Some(&pairing.first_address.as_str())
            || fields.get("Label") != Some(&label.as_str())
            || fields.get("Nonce") != Some(&id)
            || fields.get("Expires") != Some(&expires)
            || rules::identifier(fields.get("Session").copied().ok_or(BsvHomePlatformAccountError::InvalidResponse)?).is_err()
            || rules::string(&response, "rootAddress")? != pairing.first_address
            || hex::encode(sha256(message.as_bytes())) != action_hash
        {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let challenge = BsvHomePlatformHdEnrollment {
            id: id.into(),
            message: message.into(),
            action_hash: action_hash.into(),
            expires_at: expires.into(),
        };
        self.save_operation(format!("hd.{id}"), lease, json!({ "response": response, "pairing": pairing }))?;
        Ok(challenge)
    }

    pub async fn hd_enroll(&self, id: String, signature: String) -> Result<BsvHomePlatformHdAccount, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let (lease, pending) = self.pending_operation(&format!("hd.{id}"))?;
        let response = pending.get("response").ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let pairing: Pairing = serde_json::from_value(pending.get("pairing").cloned().ok_or(BsvHomePlatformAccountError::InvalidResponse)?)?;
        let message = rules::string(response, "message")?;
        rules::unexpired(rules::string(response, "expiresAt")?)?;
        if recover_bitcoin_message_address(message.as_bytes(), &signature).map_err(|_| BsvHomePlatformAccountError::InvalidInput)? != pairing.first_address {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let grant = self.authorize(rules::string(response, "actionHash")?).await?;
        self.require_lease(&lease)?;
        let account = hd_account(
            self.authenticated_json("portal/hd/enroll", json!({ "id": id, "signature": signature, "grant": grant }))
                .await?,
        )?;
        if account.base_path != pairing.account_path {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.state()?.operations.remove(&format!("hd.{id}"));
        Ok(account)
    }

    pub async fn hd_snapshot(&self, id: String, account: Account, known_external_indexes: Vec<u32>) -> Result<BsvHomePlatformHdSnapshot, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let pairing = Pairing::from_account(&account).map_err(|_| BsvHomePlatformAccountError::InvalidInput)?;
        let lease = self.session_lease()?;
        let accounts = self.hd_accounts().await?;
        let account = accounts.into_iter().find(|account| account.id == id).ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let addresses = self.hd_remote_addresses(&id).await?;
        self.require_lease(&lease)?;
        verify_account(&account, &addresses, &pairing)?;
        let balance = self.hd_balance(id.clone()).await?;
        let can_allocate = balance.state == "CURRENT" && account.next_external < 10_000 && known_external_indexes.contains(&account.next_external);
        let total = addresses.len();
        let addresses = verified_addresses(addresses, &pairing, &known_external_indexes)?;
        let last_scan_id = self.store.get(format!("{}.{}.hd.{id}", self.scope(), lease.user_id))?;
        self.require_lease(&lease)?;
        Ok(BsvHomePlatformHdSnapshot {
            account,
            balance,
            unverified_address_count: (total - addresses.len()) as u64,
            addresses,
            can_allocate,
            last_scan_id,
        })
    }

    pub async fn hd_allocate(&self, id: String, account: Account, known_external_indexes: Vec<u32>) -> Result<BsvHomePlatformHdAddress, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let pairing = Pairing::from_account(&account).map_err(|_| BsvHomePlatformAccountError::InvalidInput)?;
        let lease = self.session_lease()?;
        let account = self
            .hd_accounts()
            .await?
            .into_iter()
            .find(|account| account.id == id)
            .ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let addresses = self.hd_remote_addresses(&id).await?;
        verify_account(&account, &addresses, &pairing)?;
        if account.next_external >= 10_000 || !known_external_indexes.contains(&account.next_external) || self.hd_balance(id.clone()).await?.state != "CURRENT" {
            return Err(BsvHomePlatformAccountError::Api {
                code: "HD_ADDRESS_HISTORY_UNKNOWN".into(),
            });
        }
        self.require_lease(&lease)?;
        let allocated: BsvHomePlatformHdAddress = serde_json::from_value(self.authenticated_json("portal/hd/addresses/allocate", json!({ "id": id, "branch": 0 })).await?)?;
        if allocated.branch != 0 || allocated.index != account.next_external {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let verified = verified_addresses(vec![allocated.clone()], &pairing, &known_external_indexes)?;
        if verified != vec![allocated.clone()] {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(allocated)
    }

    pub async fn hd_balance(&self, id: String) -> Result<BsvHomePlatformHdBalance, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let response = self.authenticated_json("portal/hd/balance", json!({ "id": id })).await?;
        if hd_account(response.get("account").cloned().ok_or(BsvHomePlatformAccountError::InvalidResponse)?)?.id != id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let balance: BsvHomePlatformHdBalance = serde_json::from_value(response)?;
        if !["UNKNOWN", "CURRENT", "ERROR", "STALE", "SYNCING"].contains(&balance.state.as_str()) {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        if let Some(confirmed) = &balance.confirmed_satoshis {
            confirmed.parse::<u64>().map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
        }
        if let Some(pending) = &balance.unconfirmed_satoshis {
            pending.parse::<i64>().map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
        }
        if balance.state == "CURRENT" && (balance.confirmed_satoshis.is_none() || balance.unconfirmed_satoshis.is_none()) {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(balance)
    }

    pub async fn hd_start_scan(&self, account_id: String, gap_limit: u32, max_index: u32) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        scan_range(&account_id, gap_limit, max_index)?;
        let scan = hd_scan(
            self.authenticated_json("portal/hd/scans/start", json!({ "id": account_id, "gapLimit": gap_limit, "maxIndex": max_index }))
                .await?,
        )?;
        if scan.account_id != account_id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.save_scan(&scan)?;
        Ok(scan)
    }

    pub async fn hd_scan_status(&self, id: String) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let scan = hd_scan(self.authenticated_json("portal/hd/scans/one", json!({ "id": id })).await?)?;
        if scan.id != id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(scan)
    }

    pub async fn hd_cancel_scan(&self, id: String) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        let previous = self.hd_scan_status(id.clone()).await?;
        let scan = hd_scan(self.authenticated_json("portal/hd/scans/cancel", json!({ "id": id })).await?)?;
        if scan.id != id || scan.account_id != previous.account_id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(scan)
    }

    pub async fn hd_resume_scan(&self, id: String, max_index: u32) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
        let previous = self.hd_scan_status(id.clone()).await?;
        scan_range(&id, previous.gap_limit, max_index)?;
        if !["LIMIT_REACHED", "CANCELLED", "RETRY"].contains(&previous.status.as_str()) || max_index < previous.max_index || max_index <= previous.next_index {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let scan = hd_scan(
            self.authenticated_json("portal/hd/scans/resume", json!({ "id": id, "gapLimit": previous.gap_limit, "maxIndex": max_index }))
                .await?,
        )?;
        if scan.id != id || scan.account_id != previous.account_id || scan.gap_limit != previous.gap_limit {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.save_scan(&scan)?;
        Ok(scan)
    }
}

impl BsvHomePlatformAccountService {
    async fn hd_remote_addresses(&self, id: &str) -> Result<Vec<BsvHomePlatformHdAddress>, BsvHomePlatformAccountError> {
        let addresses: Vec<BsvHomePlatformHdAddress> = serde_json::from_value(self.authenticated_json("portal/hd/addresses", json!({ "id": id })).await?)?;
        for address in &addresses {
            if address.branch > 1 || address.index >= 1_000_000 {
                return Err(BsvHomePlatformAccountError::InvalidResponse);
            }
        }
        Ok(addresses)
    }

    fn save_scan(&self, scan: &BsvHomePlatformHdScan) -> Result<(), BsvHomePlatformAccountError> {
        let lease = self.session_lease()?;
        self.store.set(format!("{}.{}.hd.{}", self.scope(), lease.user_id, scan.account_id), scan.id.clone())?;
        self.require_lease(&lease)
    }
}

fn hd_account(value: Value) -> Result<BsvHomePlatformHdAccount, BsvHomePlatformAccountError> {
    if rules::string(&value, "ancestorPathVerification")? != "CLIENT_DECLARED"
        || !rules::boolean(&value, "publicDerivationVerified")?
        || rules::string(&value, "backupResponsibility")? != "CLIENT_ONLY"
        || rules::string(&value, "status")? != "ACTIVE"
    {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    let account: BsvHomePlatformHdAccount = serde_json::from_value(value)?;
    rules::identifier(&account.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    if account.next_external > 1_000_000 || account.next_internal > 1_000_000 {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(account)
}

fn hd_scan(value: Value) -> Result<BsvHomePlatformHdScan, BsvHomePlatformAccountError> {
    let mut scan: BsvHomePlatformHdScan = serde_json::from_value(value)?;
    rules::identifier(&scan.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    scan_range(&scan.account_id, scan.gap_limit, scan.max_index).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    if !["QUEUED", "RUNNING", "RETRY", "COMPLETE", "CANCELLED", "LIMIT_REACHED"].contains(&scan.status.as_str()) || scan.branch > 1 {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    scan.error_code = scan.error_code.map(|code| rules::public_code(&code));
    scan.actions.push(BsvHomePlatformHdScanAction::Refresh);
    match scan.status.as_str() {
        "QUEUED" | "RUNNING" => scan.actions.push(BsvHomePlatformHdScanAction::Cancel),
        "RETRY" => scan.actions.extend([BsvHomePlatformHdScanAction::Cancel, BsvHomePlatformHdScanAction::Resume]),
        "CANCELLED" | "LIMIT_REACHED" => scan.actions.push(BsvHomePlatformHdScanAction::Resume),
        "COMPLETE" => {}
        _ => return Err(BsvHomePlatformAccountError::InvalidResponse),
    }
    Ok(scan)
}

fn scan_range(id: &str, gap: u32, maximum: u32) -> Result<(), BsvHomePlatformAccountError> {
    rules::identifier(id)?;
    if !(1..=100).contains(&gap) || !(20..=10_000).contains(&maximum) {
        return Err(BsvHomePlatformAccountError::InvalidInput);
    }
    Ok(())
}

fn enrollment_fields(message: &str) -> Result<HashMap<&str, &str>, BsvHomePlatformAccountError> {
    let lines = message.split('\n').collect::<Vec<_>>();
    if lines.len() != 9 || lines[0] != "WowPay HD Enrollment/v1" {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    let mut fields = HashMap::new();
    for line in &lines[1..] {
        let (key, value) = line.split_once(": ").ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        if fields.insert(key, value).is_some() {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
    }
    Ok(fields)
}

fn verify_account(account: &BsvHomePlatformHdAccount, addresses: &[BsvHomePlatformHdAddress], pairing: &Pairing) -> Result<(), BsvHomePlatformAccountError> {
    let roots = addresses
        .iter()
        .filter(|address| Some(&address.wallet_id) == account.root_wallet_id.as_ref() && address.branch == 0 && address.index == 0)
        .collect::<Vec<_>>();
    if roots.len() != 1 || account.base_path != pairing.account_path || roots[0].address != pairing.first_address {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(())
}

fn verified_addresses(addresses: Vec<BsvHomePlatformHdAddress>, pairing: &Pairing, indexes: &[u32]) -> Result<Vec<BsvHomePlatformHdAddress>, BsvHomePlatformAccountError> {
    let mut verified = Vec::new();
    for address in addresses {
        if address.branch != 0 || address.index >= 10_000 || !indexes.contains(&address.index) {
            continue;
        }
        let derived = pairing.address(0, address.index).map_err(|_| BsvHomePlatformAccountError::InvalidInput)?;
        let public_key = hex::decode(&address.public_key).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
        let encoded = BsvAddress::from_public_key(&public_key).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?.encode();
        if derived.address != address.address || encoded != address.address {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        verified.push(address);
    }
    Ok(verified)
}

#[cfg(test)]
mod tests {
    use bitcoin::{
        NetworkKind,
        bip32::{ChildNumber, Xpriv, Xpub},
        secp256k1::Secp256k1,
    };

    use super::*;

    fn fixture() -> (Pairing, Vec<BsvHomePlatformHdAddress>) {
        let curve = Secp256k1::new();
        let root = Xpriv::new_master(NetworkKind::Main, &[7; 32]).unwrap();
        let account = root
            .derive_priv(
                &curve,
                &[
                    ChildNumber::from_hardened_idx(44).unwrap(),
                    ChildNumber::from_hardened_idx(236).unwrap(),
                    ChildNumber::from_hardened_idx(0).unwrap(),
                ],
            )
            .unwrap();
        let xpub = Xpub::from_priv(&curve, &account);
        let addresses = [0, 1, 2]
            .into_iter()
            .map(|index| {
                let key = xpub
                    .derive_pub(&curve, &[ChildNumber::Normal { index: 0 }, ChildNumber::Normal { index }])
                    .unwrap()
                    .public_key;
                BsvHomePlatformHdAddress {
                    wallet_id: format!("wallet-{index}"),
                    branch: 0,
                    index,
                    address: BsvAddress::from_public_key(&key.serialize()).unwrap().encode(),
                    public_key: hex::encode(key.serialize()),
                }
            })
            .collect::<Vec<_>>();
        let account = Account {
            chain: primitives::Chain::BitcoinSv,
            address: addresses[0].address.clone(),
            derivation_path: "m/44'/236'/0'/0/0".into(),
            extended_public_key: Some(xpub.to_string()),
        };
        (Pairing::from_account(&account).unwrap(), addresses)
    }

    #[test]
    fn test_hd_receiving_requires_independent_derivation_and_known_external_range() {
        let (pairing, addresses) = fixture();
        assert_eq!(verified_addresses(addresses.clone(), &pairing, &[0, 1]).unwrap(), addresses[..2]);
        let mut changed = addresses[0].clone();
        changed.address = addresses[1].address.clone();
        assert_eq!(verified_addresses(vec![changed], &pairing, &[0]), Err(BsvHomePlatformAccountError::InvalidResponse));
        let mut changed = addresses[0].clone();
        changed.public_key = addresses[1].public_key.clone();
        assert_eq!(verified_addresses(vec![changed], &pairing, &[0]), Err(BsvHomePlatformAccountError::InvalidResponse));
        let mut change_branch = addresses[0].clone();
        change_branch.branch = 1;
        assert_eq!(verified_addresses(vec![change_branch], &pairing, &[0]).unwrap(), Vec::<BsvHomePlatformHdAddress>::new());
        let account = BsvHomePlatformHdAccount {
            id: "1".repeat(32),
            label: "test".into(),
            base_path: pairing.account_path.clone(),
            root_wallet_id: Some(addresses[0].wallet_id.clone()),
            next_external: 1,
            next_internal: 0,
        };
        assert_eq!(verify_account(&account, &addresses, &pairing), Ok(()));
        let mut duplicated = addresses.clone();
        duplicated.push(addresses[0].clone());
        assert_eq!(verify_account(&account, &duplicated, &pairing), Err(BsvHomePlatformAccountError::InvalidResponse));
        let mut different = account;
        different.base_path = "m/44'/145'/0'".into();
        assert_eq!(verify_account(&different, &addresses, &pairing), Err(BsvHomePlatformAccountError::InvalidResponse));
    }

    #[test]
    fn test_hd_enrollment_rejects_duplicate_fields_and_scan_limits() {
        assert_eq!(
            enrollment_fields("WowPay HD Enrollment/v1\nUser: 42\nSession: 1\nAccount XPUB: x\nBase Path: m\nAddress: x\nLabel: y\nNonce: z\nNonce: z"),
            Err(BsvHomePlatformAccountError::InvalidResponse)
        );
        assert_eq!(scan_range(&"1".repeat(32), 20, 1000), Ok(()));
        assert_eq!(scan_range(&"1".repeat(32), 101, 1000), Err(BsvHomePlatformAccountError::InvalidInput));
        assert_eq!(scan_range(&"1".repeat(32), 20, 10001), Err(BsvHomePlatformAccountError::InvalidInput));
    }

    #[test]
    fn test_hd_scan_error_codes_cannot_expose_raw_server_details() {
        let mut scan = json!({
            "id": "1".repeat(32), "accountId": "2".repeat(32), "status": "RETRY", "branch": 0,
            "nextIndex": 21, "gapLimit": 20, "maxIndex": 1000,
            "errorCode": "provider failed: Authorization=server-internal-token seed=fixture-only-not-a-secret",
        });
        assert_eq!(hd_scan(scan.clone()).unwrap().error_code.as_deref(), Some("SERVICE_ERROR"));
        scan["errorCode"] = "HD_SCAN_RANGE_INVALID".into();
        assert_eq!(hd_scan(scan.clone()).unwrap().error_code.as_deref(), Some("HD_SCAN_RANGE_INVALID"));
        scan["errorCode"] = Value::Null;
        assert_eq!(hd_scan(scan).unwrap().error_code, None);
    }
}
