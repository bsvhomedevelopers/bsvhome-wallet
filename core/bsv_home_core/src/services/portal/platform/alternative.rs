use bsv_home_hash::sha2::sha256;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::services::offline_identity::BsvHomeOfflineIdentityService;

use super::rules;
use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountService, BsvHomePlatformAlternativeAction, BsvHomePlatformAlternativeChallenge, BsvHomePlatformAlternativeKey,
    BsvHomePlatformAlternativeRequest, BsvHomePlatformPasskeyOperation,
};

const POLICY: &str = "ALT_IDENTITY_PRE_ENROLLED_P1_V1";
const PREFIX: &str = "portal/identity/alternative/";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OfflineProof {
    version: u32,
    kind: String,
    id: String,
    purpose: String,
    action_hash: String,
    public_key: String,
    signature: String,
}

impl BsvHomePlatformAccountService {
    pub async fn alternative_keys(&self) -> Result<Vec<BsvHomePlatformAlternativeKey>, BsvHomePlatformAccountError> {
        let value = self.authenticated_json(&format!("{PREFIX}keys"), json!({})).await?;
        value
            .as_array()
            .ok_or(BsvHomePlatformAccountError::InvalidResponse)?
            .iter()
            .cloned()
            .map(alternative_key)
            .collect()
    }

    pub async fn alternative_enrollment(&self, public_key: String, label: String) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
        validate_public_key(&public_key)?;
        rules::label(&label)?;
        let lease = self.session_lease()?;
        let response = self
            .authenticated_json(&format!("{PREFIX}keys/enroll/challenge"), json!({ "publicKey": public_key, "label": label }))
            .await?;
        let challenge = alternative_challenge(&response, "Enroll", public_key, label, lease.user_id.clone())?;
        self.save_operation(
            format!("alternative.{}", challenge.id),
            lease,
            json!({ "challenge": response, "publicKey": challenge.public_key, "label": challenge.label, "purpose": "Enroll" }),
        )?;
        Ok(challenge)
    }

    pub async fn alternative_enroll(&self, id: String, proof_json: String) -> Result<BsvHomePlatformAlternativeKey, BsvHomePlatformAccountError> {
        let (lease, pending) = self.pending_operation(&format!("alternative.{id}"))?;
        let proof = self.offline_proof(&id, &pending, &proof_json)?;
        let action = rules::string(pending.get("challenge").ok_or(BsvHomePlatformAccountError::InvalidResponse)?, "actionHash")?;
        let grant = self.authorize(action).await?;
        self.require_lease(&lease)?;
        let key = alternative_key(
            self.authenticated_json(&format!("{PREFIX}keys/enroll"), json!({ "id": id, "signature": proof.signature, "grant": grant }))
                .await?,
        )?;
        if key.id != id || key.status != "ACTIVE" || key.fingerprint != public_key_fingerprint(&proof.public_key)? {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.state()?.operations.remove(&format!("alternative.{id}"));
        Ok(key)
    }

    pub async fn alternative_revoke(&self, id: String) -> Result<(), BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let lease = self.session_lease()?;
        let response = self.authenticated_json(&format!("{PREFIX}keys/revoke/challenge"), json!({ "id": id })).await?;
        let grant = self.authorize(rules::string(&response, "actionHash")?).await?;
        self.require_lease(&lease)?;
        let result = self.authenticated_json(&format!("{PREFIX}keys/revoke"), json!({ "id": id, "grant": grant })).await?;
        if !rules::boolean(&result, "revoked")? {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(())
    }

    pub async fn alternative_recovery(
        &self,
        key_id: String,
        public_key: String,
        device_public_key: String,
    ) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
        rules::identifier(&key_id)?;
        validate_public_key(&public_key)?;
        validate_public_key(&device_public_key)?;
        let lease = self.session_lease()?;
        let keys = self.alternative_keys().await?;
        let key = keys
            .into_iter()
            .find(|key| key.id == key_id && key.status == "ACTIVE")
            .ok_or(BsvHomePlatformAccountError::InvalidInput)?;
        if key.fingerprint != public_key_fingerprint(&public_key)? {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let response = self
            .authenticated_json(&format!("{PREFIX}recovery/challenge"), json!({ "keyId": key_id, "devicePublicKey": device_public_key }))
            .await?;
        let challenge = alternative_challenge(&response, "Recover", public_key.clone(), key.label.clone(), lease.user_id.clone())?;
        let pending = json!({ "challenge": response, "publicKey": public_key, "devicePublicKey": device_public_key, "keyId": key_id, "label": key.label, "purpose": "Recover" });
        self.save_operation(format!("alternative.{}", challenge.id), lease, pending)?;
        Ok(challenge)
    }

    pub async fn alternative_request(&self, id: String, proof_json: String, device_signature: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        let (lease, pending) = self.pending_operation(&format!("alternative.{id}"))?;
        let proof = self.offline_proof(&id, &pending, &proof_json)?;
        if proof.purpose != "Recover" {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        self.verify_device_proof(&pending, &proof.action_hash, &device_signature)?;
        self.require_lease(&lease)?;
        self.store.set(
            self.alternative_pending_key(&lease.user_id),
            serde_json::to_string(&json!({ "id": id, "pending": pending }))?,
        )?;
        self.require_lease(&lease)?;
        let request = alternative_request(
            self.authenticated_json(
                &format!("{PREFIX}recovery/request"),
                json!({ "id": id, "signature": proof.signature, "deviceSignature": device_signature }),
            )
            .await?,
        )?;
        if request.id != id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(request)
    }

    pub async fn alternative_pending(&self) -> Result<Option<BsvHomePlatformAlternativeRequest>, BsvHomePlatformAccountError> {
        let lease = self.session_lease()?;
        let Some(pending) = self.store.get(self.alternative_pending_key(&lease.user_id))? else {
            return Ok(None);
        };
        let record: Value = serde_json::from_str(&pending)?;
        self.require_lease(&lease)?;
        self.alternative_status(rules::string(&record, "id")?.into()).await.map(Some)
    }

    pub async fn alternative_status(&self, id: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let request = alternative_request(self.authenticated_json(&format!("{PREFIX}recovery/status"), json!({ "id": id })).await?)?;
        if request.id != id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(request)
    }

    pub async fn alternative_cancel(&self, id: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let request = alternative_request(self.authenticated_json(&format!("{PREFIX}recovery/cancel"), json!({ "id": id })).await?)?;
        if request.id != id || request.status != "CANCELLED" {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(request)
    }

    pub async fn alternative_registration(&self, id: String) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let lease = self.session_lease()?;
        let stored = self
            .store
            .get(self.alternative_pending_key(&lease.user_id))?
            .ok_or(BsvHomePlatformAccountError::InvalidInput)?;
        let stored: Value = serde_json::from_str(&stored)?;
        if rules::string(&stored, "id")? != id {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let mut pending = stored.get("pending").cloned().ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let response = self.authenticated_json(&format!("{PREFIX}recovery/registration/challenge"), json!({ "id": id })).await?;
        let challenge = alternative_challenge(
            &response,
            "Register",
            rules::string(&pending, "publicKey")?.into(),
            rules::string(&pending, "label")?.into(),
            lease.user_id.clone(),
        )?;
        if challenge.id != id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        pending["challenge"] = response;
        pending["purpose"] = "Register".into();
        self.save_operation(format!("alternative.{id}"), lease, pending)?;
        Ok(challenge)
    }

    pub async fn alternative_register(&self, id: String, proof_json: String, device_signature: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        let (lease, pending) = self.pending_operation(&format!("alternative.{id}"))?;
        let proof = self.offline_proof(&id, &pending, &proof_json)?;
        if proof.purpose != "Register" {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        self.verify_device_proof(&pending, &proof.action_hash, &device_signature)?;
        let mut body = self.device();
        body["id"] = id.clone().into();
        body["signature"] = proof.signature.into();
        body["deviceSignature"] = device_signature.into();
        body["label"] = "iOS".into();
        let start = self.authenticated_json(&format!("{PREFIX}recovery/registration/start"), body).await?;
        let finish = self.passkey_response(BsvHomePlatformPasskeyOperation::Register, start).await?;
        self.require_lease(&lease)?;
        self.authenticated_json("portal/identity/passkey/register/finish", finish).await?;
        let request = self.alternative_status(id).await?;
        if request.status != "COMPLETE" {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(request)
    }

    pub fn alternative_challenge_json(&self, id: String) -> Result<String, BsvHomePlatformAccountError> {
        let (lease, pending) = self.pending_operation(&format!("alternative.{id}"))?;
        let challenge = pending.get("challenge").ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        rules::unexpired(rules::string(challenge, "expiresAt")?)?;
        let public_key = rules::string(&pending, "publicKey")?;
        serde_json::to_string(&json!({ "version": 1, "kind": "wowpay-offline-identity-challenge", "id": id,
            "actionHash": rules::string(challenge, "actionHash")?, "purpose": rules::string(&pending, "purpose")?,
            "expiresAt": rules::string(challenge, "expiresAt")?, "userId": lease.user_id,
            "label": rules::string(&pending, "label")?, "fingerprint": public_key_fingerprint(public_key)?,
        }))
        .map_err(Into::into)
    }
}

impl BsvHomePlatformAccountService {
    fn alternative_pending_key(&self, user_id: &str) -> String {
        format!("platform.{}.user.{user_id}.alternative", hex::encode(sha256(self.scope().as_bytes())))
    }

    pub(super) fn validate_proof(&self, id: &str, proof_json: &str) -> Result<(), BsvHomePlatformAccountError> {
        let (_, pending) = self.pending_operation(&format!("alternative.{id}"))?;
        self.offline_proof(id, &pending, proof_json)?;
        Ok(())
    }

    fn offline_proof(&self, id: &str, pending: &Value, proof_json: &str) -> Result<OfflineProof, BsvHomePlatformAccountError> {
        if proof_json.len() > 16_384 {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let proof: OfflineProof = serde_json::from_str(proof_json)?;
        let challenge = pending.get("challenge").ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        rules::unexpired(rules::string(challenge, "expiresAt")?)?;
        if proof.version != 1
            || proof.kind != "wowpay-offline-identity-proof"
            || proof.id != id
            || proof.purpose != rules::string(pending, "purpose")?
            || proof.action_hash != rules::string(challenge, "actionHash")?
            || proof.public_key != rules::string(pending, "publicKey")?
        {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        validate_public_key(&proof.public_key)?;
        if !self
            .authenticator
            .verify_offline_proof(proof.public_key.clone(), proof.action_hash.clone(), proof.signature.clone())?
        {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        Ok(proof)
    }

    fn verify_device_proof(&self, pending: &Value, action: &str, signature: &str) -> Result<(), BsvHomePlatformAccountError> {
        let public_key = rules::string(pending, "devicePublicKey")?;
        if !self.authenticator.verify_offline_proof(public_key.into(), action.into(), signature.into())? {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        Ok(())
    }
}

fn public_key_fingerprint(public_key: &str) -> Result<String, BsvHomePlatformAccountError> {
    BsvHomeOfflineIdentityService::new()
        .public_key_fingerprint(public_key.into())
        .map_err(|_| BsvHomePlatformAccountError::InvalidInput)
}

fn validate_public_key(public_key: &str) -> Result<(), BsvHomePlatformAccountError> {
    public_key_fingerprint(public_key).map(|_| ())
}

fn policy(value: &Value) -> Result<(), BsvHomePlatformAccountError> {
    if rules::string(value, "policyVersion")? != POLICY {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(())
}

fn alternative_challenge(
    value: &Value,
    purpose: &str,
    public_key: String,
    label: String,
    user_id: String,
) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
    policy(value)?;
    let id = rules::string(value, "id")?;
    rules::identifier(id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    let action = rules::string(value, "actionHash")?;
    rules::action_hash(action)?;
    let expires_at = rules::string(value, "expiresAt")?;
    rules::unexpired(expires_at)?;
    Ok(BsvHomePlatformAlternativeChallenge {
        id: id.into(),
        action_hash: action.into(),
        expires_at: expires_at.into(),
        purpose: purpose.into(),
        fingerprint: public_key_fingerprint(&public_key)?,
        public_key,
        label,
        user_id,
    })
}

fn alternative_key(value: Value) -> Result<BsvHomePlatformAlternativeKey, BsvHomePlatformAccountError> {
    policy(&value)?;
    if rules::boolean(&value, "fundsAuthority")? {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    let separate = value
        .get("storeSeparatelyFrom")
        .and_then(Value::as_array)
        .ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
    if separate.len() != 3
        || !["DEVICE", "MPC_BACKUP", "MPC_RECOVERY_KEY"]
            .iter()
            .all(|expected| separate.iter().any(|actual| actual.as_str() == Some(expected)))
    {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    let key: BsvHomePlatformAlternativeKey = serde_json::from_value(value)?;
    rules::identifier(&key.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    rules::action_hash(&key.fingerprint)?;
    if key.status != "ACTIVE" && key.status != "REVOKED" {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(key)
}

fn alternative_request(value: Value) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
    policy(&value)?;
    if !rules::boolean(&value, "mpcRecoveryStillRequired")? {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    let mut request: BsvHomePlatformAlternativeRequest = serde_json::from_value(value)?;
    rules::identifier(&request.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    if request.required != 2 || request.approved > 2 || !["WAIT_REVIEW", "APPROVED", "REJECTED", "CANCELLED", "EXPIRED", "COMPLETE"].contains(&request.status.as_str()) {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    request.actions.push(BsvHomePlatformAlternativeAction::Refresh);
    match request.status.as_str() {
        "WAIT_REVIEW" => request.actions.push(BsvHomePlatformAlternativeAction::Cancel),
        "APPROVED" => request
            .actions
            .extend([BsvHomePlatformAlternativeAction::Cancel, BsvHomePlatformAlternativeAction::Register]),
        "REJECTED" | "CANCELLED" | "EXPIRED" | "COMPLETE" => {}
        _ => return Err(BsvHomePlatformAccountError::InvalidResponse),
    }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    use super::super::testkit::{MemoryPlatformAuthenticator, MemoryPlatformProvider, MemoryPlatformSecureStore};
    use super::*;

    #[test]
    fn test_alternative_identity_retains_two_reviewers_and_mpc_recovery_requirement() {
        let base: Value = serde_json::from_str(r#"{"policyVersion":"ALT_IDENTITY_PRE_ENROLLED_P1_V1","mpcRecoveryStillRequired":true,"id":"11111111111111111111111111111111","status":"WAIT_REVIEW","approvedReviewCount":0,"requiredIndependentReviewers":2,"notificationAccepted":false,"readyAt":null,"expiresAt":"2099-01-01T00:00:00Z"}"#).unwrap();
        let request = alternative_request(base.clone()).unwrap();
        assert_eq!((request.required, request.approved, request.notified), (2, 0, false));
        let mut single = base.clone();
        single["requiredIndependentReviewers"] = 1.into();
        assert_eq!(alternative_request(single), Err(BsvHomePlatformAccountError::InvalidResponse));
        let mut bypass = base.clone();
        bypass["mpcRecoveryStillRequired"] = false.into();
        assert_eq!(alternative_request(bypass), Err(BsvHomePlatformAccountError::InvalidResponse));
        let mut unknown = base;
        unknown["status"] = "AUTOMATICALLY_APPROVED".into();
        assert_eq!(alternative_request(unknown), Err(BsvHomePlatformAccountError::InvalidResponse));
    }

    #[test]
    fn test_alternative_key_cannot_claim_funds_authority_or_change_separation_policy() {
        let base: Value = serde_json::from_str(r#"{"policyVersion":"ALT_IDENTITY_PRE_ENROLLED_P1_V1","fundsAuthority":false,"storeSeparatelyFrom":["DEVICE","MPC_BACKUP","MPC_RECOVERY_KEY"],"id":"11111111111111111111111111111111","label":"offline","status":"ACTIVE","fingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#).unwrap();
        assert_eq!(alternative_key(base.clone()).unwrap().status, "ACTIVE");
        let mut authority = base.clone();
        authority["fundsAuthority"] = true.into();
        assert_eq!(alternative_key(authority), Err(BsvHomePlatformAccountError::InvalidResponse));
        let mut storage = base;
        storage["storeSeparatelyFrom"] = serde_json::from_str(r#"["DEVICE","MPC_BACKUP"]"#).unwrap();
        assert_eq!(alternative_key(storage), Err(BsvHomePlatformAccountError::InvalidResponse));
    }

    #[test]
    fn test_offline_proof_binds_every_challenge_field_before_independent_verification() {
        let auth = Arc::new(MemoryPlatformAuthenticator::default());
        *auth.verify_result.lock().unwrap() = true;
        let service = BsvHomePlatformAccountService::new(
            Arc::new(MemoryPlatformProvider::default()),
            Arc::new(MemoryPlatformSecureStore::default()),
            auth.clone(),
            "https://test.example/v2/api/".into(),
            "en".into(),
            "test-installation".into(),
        )
        .unwrap();
        let public_key = URL_SAFE_NO_PAD.encode(hex::decode("3059301306072a8648ce3d020106082a8648ce3d030107034200046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5").unwrap());
        let mut pending: Value = serde_json::from_str(r#"{"publicKey":"","purpose":"Recover","challenge":{"actionHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","expiresAt":"2099-01-01T00:00:00Z"}}"#).unwrap();
        pending["publicKey"] = public_key.clone().into();
        let mut proof: Value = serde_json::from_str(r#"{"version":1,"kind":"wowpay-offline-identity-proof","id":"11111111111111111111111111111111","purpose":"Recover","actionHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","publicKey":"","signature":"test-proof"}"#).unwrap();
        proof["publicKey"] = public_key.into();
        assert_eq!(service.offline_proof(&"1".repeat(32), &pending, &proof.to_string()).unwrap().id, "1".repeat(32));
        for (field, change) in [
            ("purpose", "Enroll"),
            ("actionHash", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            ("id", "22222222222222222222222222222222"),
            ("publicKey", "different-key"),
        ] {
            let mut changed = proof.clone();
            changed[field] = change.into();
            assert_eq!(
                service.offline_proof(&"1".repeat(32), &pending, &changed.to_string()).err(),
                Some(BsvHomePlatformAccountError::InvalidInput)
            );
        }
        *auth.verify_result.lock().unwrap() = false;
        assert_eq!(
            service.offline_proof(&"1".repeat(32), &pending, &proof.to_string()).err(),
            Some(BsvHomePlatformAccountError::InvalidInput)
        );
    }
}
