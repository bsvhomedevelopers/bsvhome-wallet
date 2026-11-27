use serde_json::{Value, json};

use super::rules;
use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountService, BsvHomePlatformClosure, BsvHomePlatformClosureStatus, BsvHomePlatformEmailChallenge,
    BsvHomePlatformPasskeyOperation, BsvHomePlatformSession,
};

const CLOSURE_POLICY: &str = "ACCOUNT_CLOSURE_P1_V1";

#[uniffi::export]
impl BsvHomePlatformAccountService {
    pub async fn closure_status(&self) -> Result<Option<BsvHomePlatformClosure>, BsvHomePlatformAccountError> {
        let data = self.authenticated_json("portal/account/closure/status", json!({})).await?;
        validate_policy(data.get("policy").ok_or(BsvHomePlatformAccountError::InvalidResponse)?)?;
        let request = data.get("request").and_then(Value::as_object).ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        if request.is_empty() {
            return Ok(None);
        }
        Ok(Some(closure(Value::Object(request.clone()))?))
    }

    pub async fn closure_request(&self, acknowledge_address: bool, acknowledge_retention: bool) -> Result<BsvHomePlatformClosure, BsvHomePlatformAccountError> {
        acknowledgements(acknowledge_address, acknowledge_retention)?;
        let lease = self.session_lease()?;
        let action_hash = self.closure_challenge("REQUEST", None).await?;
        let grant = self.authorize(&action_hash).await?;
        self.require_lease(&lease)?;
        closure(
            self.authenticated_json(
                "portal/account/closure/request",
                json!({
                    "kind": "REQUEST", "acknowledgeIrreversibleAddress": true, "acknowledgeRetention": true, "grant": grant,
                }),
            )
            .await?,
        )
    }

    pub async fn closure_cancel(&self, id: String) -> Result<BsvHomePlatformClosure, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let result = closure(self.authenticated_json("portal/account/closure/cancel", json!({ "id": id })).await?)?;
        if result.id != id || result.status != BsvHomePlatformClosureStatus::Cancelled {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(result)
    }

    pub async fn closure_finish(&self, id: String, acknowledge_address: bool, acknowledge_retention: bool) -> Result<BsvHomePlatformClosure, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        acknowledgements(acknowledge_address, acknowledge_retention)?;
        let lease = self.session_lease()?;
        let action_hash = self.closure_challenge("FINALIZE", Some(&id)).await?;
        let grant = self.authorize(&action_hash).await?;
        self.require_lease(&lease)?;
        let result = closure(
            self.authenticated_json(
                "portal/account/closure/finish",
                json!({
                    "id": id, "kind": "FINALIZE", "acknowledgeIrreversibleAddress": true, "acknowledgeRetention": true, "grant": grant,
                }),
            )
            .await?,
        )?;
        if result.id != id || result.status != BsvHomePlatformClosureStatus::Closed {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.clear_for(&lease)?;
        Ok(result)
    }

    pub async fn reopen_start(&self) -> Result<BsvHomePlatformEmailChallenge, BsvHomePlatformAccountError> {
        let generation = self.generation()?;
        let start = self.client.request("portal/account/closure/reopen/passkey/start", self.device(), None).await?;
        let body = self.passkey_response(BsvHomePlatformPasskeyOperation::Login, start).await?;
        if self.generation()? != generation {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        let response = self.client.request("portal/account/closure/reopen/passkey/finish", body, None).await?;
        if rules::string(&response, "nextFactor")? != "VERIFIED_EMAIL_OTP" || rules::boolean(&response, "sessionIssued")? || response.get("token").is_some() {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let challenge: BsvHomePlatformEmailChallenge = serde_json::from_value(response)?;
        rules::email_challenge(&challenge)?;
        let mut state = self.state()?;
        if state.generation != generation {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        state.email.clear();
        state.email.insert(challenge.id.clone(), (generation, true));
        Ok(challenge)
    }

    pub async fn reopen_finish(&self, challenge_id: String, code: String) -> Result<BsvHomePlatformSession, BsvHomePlatformAccountError> {
        self.email_finish_kind(challenge_id, code, true).await
    }

    pub async fn change_email_start(&self, email: String) -> Result<BsvHomePlatformEmailChallenge, BsvHomePlatformAccountError> {
        let email = rules::email(&email)?;
        let lease = self.session_lease()?;
        let action = self
            .authenticated_json("portal/account/actions/challenge", json!({ "kind": "EMAIL_CHANGE", "email": email }))
            .await?;
        let grant = self.authorize(rules::string(&action, "actionHash")?).await?;
        self.require_lease(&lease)?;
        let response = self
            .authenticated_json("portal/account/email/change/start", json!({ "email": email, "grant": grant }))
            .await?;
        let challenge: BsvHomePlatformEmailChallenge = serde_json::from_value(response)?;
        rules::email_challenge(&challenge)?;
        if challenge.retry_after_seconds.is_none() {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.save_operation(format!("email.{}", challenge.id), lease, json!({ "email": email }))?;
        Ok(challenge)
    }

    pub async fn change_email_finish(&self, id: String, code: String) -> Result<super::BsvHomePlatformProfile, BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        rules::code(&code)?;
        let (lease, pending) = self.pending_operation(&format!("email.{id}"))?;
        let expected = rules::string(&pending, "email")?;
        let response = self.authenticated_json("portal/account/email/change/finish", json!({ "id": id, "code": code })).await?;
        self.require_lease(&lease)?;
        if rules::string(&response, "email")? != expected || !rules::boolean(&response, "emailVerified")? {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let profile = self.profile().await?;
        if profile.email != expected {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.state()?.operations.remove(&format!("email.{id}"));
        Ok(profile)
    }
}

impl BsvHomePlatformAccountService {
    async fn closure_challenge(&self, kind: &str, id: Option<&str>) -> Result<String, BsvHomePlatformAccountError> {
        let mut body = json!({ "kind": kind });
        if let Some(id) = id {
            body["id"] = id.into();
        }
        let response = self.authenticated_json("portal/account/closure/challenge", body).await?;
        validate_policy(response.get("policy").ok_or(BsvHomePlatformAccountError::InvalidResponse)?)?;
        let hash = rules::string(&response, "actionHash")?;
        rules::action_hash(hash)?;
        Ok(hash.into())
    }
}

fn acknowledgements(address: bool, retention: bool) -> Result<(), BsvHomePlatformAccountError> {
    if !address || !retention {
        return Err(BsvHomePlatformAccountError::Api {
            code: "CLOSURE_ACKNOWLEDGEMENT_REQUIRED".into(),
        });
    }
    Ok(())
}

fn validate_policy(policy: &Value) -> Result<(), BsvHomePlatformAccountError> {
    if rules::string(policy, "version")? != CLOSURE_POLICY
        || !rules::boolean(policy, "chainAddressesRemainValid")?
        || !rules::boolean(policy, "walletOwnershipRetained")?
        || !rules::boolean(policy, "recoveryMaterialsRetained")?
        || rules::boolean(policy, "deletionScheduled")?
    {
        return Err(BsvHomePlatformAccountError::Api {
            code: "CLOSURE_POLICY_INVALID".into(),
        });
    }
    Ok(())
}

fn closure(value: Value) -> Result<BsvHomePlatformClosure, BsvHomePlatformAccountError> {
    let closure: BsvHomePlatformClosure = serde_json::from_value(value)?;
    rules::identifier(&closure.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    if closure.policy_version != CLOSURE_POLICY {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(closure)
}
