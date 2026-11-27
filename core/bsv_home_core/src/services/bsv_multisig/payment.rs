use std::sync::LazyLock;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bsv_home_bsv::{
    BsvAddress,
    multisig::{Policy, parse_locking_script, policy_descriptor},
};
use number_formatter::BigNumberFormatter;
use primitives::{Address, AssetId, Chain, PaymentURLDecoder};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigService, rules};
use crate::models::payment::{BsvHomePayment, BsvHomePaymentRequest};
use crate::payment::{BsvHomePaymentDestination, BsvHomePaymentRecipient, BsvHomePaymentWalletAsset, payment_transfer_destination};
use crate::services::transfer::model::BsvHomeRecipient;

#[cfg(test)]
#[path = "payment_tests.rs"]
mod tests;

const PAYMENT_PREFIX: &str = "bsvhome://multisig/pay?v=1&request=";
const MAX_ENCODED_REQUEST: usize = 1600;
const MAX_NAME_LENGTH: usize = 80;
const MAX_AMOUNT_LENGTH: usize = 18;
static UNSAFE_NAME: LazyLock<Result<Regex, Error>> = LazyLock::new(|| Regex::new(r"[\p{Cc}\p{Cf}]").map_err(|_| Error::invalid("INVALID_MULTISIG_PAYMENT")));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BsvHomeBsvMultisigPaymentRequest {
    pub name: String,
    pub threshold: u32,
    pub public_keys: Vec<String>,
    pub fingerprint: String,
    pub amount: String,
}

#[uniffi::export]
pub fn parse_bsv_multisig_payment_request(value: String) -> Result<BsvHomeBsvMultisigPaymentRequest, Error> {
    let encoded = value.trim().strip_prefix(PAYMENT_PREFIX).ok_or_else(|| Error::invalid("INVALID_MULTISIG_PAYMENT"))?;
    if encoded.is_empty() || encoded.len() > MAX_ENCODED_REQUEST || !encoded.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-') {
        return Err(Error::invalid("INVALID_MULTISIG_PAYMENT"));
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| Error::invalid("INVALID_MULTISIG_PAYMENT"))?;
    let (name, threshold, public_keys, fingerprint, amount) =
        serde_json::from_slice::<(String, u32, Vec<String>, String, String)>(&bytes).map_err(|_| Error::invalid("INVALID_MULTISIG_PAYMENT"))?;
    let request = BsvHomeBsvMultisigPaymentRequest {
        name,
        threshold,
        public_keys,
        fingerprint,
        amount,
    };
    request.validate()?;
    Ok(request)
}

#[uniffi::export]
pub fn bsv_multisig_payment_request_qr_payload(request: BsvHomeBsvMultisigPaymentRequest) -> Result<String, Error> {
    request.validate()?;
    let payload = serde_json::to_vec(&(&request.name, request.threshold, &request.public_keys, &request.fingerprint, &request.amount))?;
    let encoded = URL_SAFE_NO_PAD.encode(payload);
    if encoded.len() > MAX_ENCODED_REQUEST {
        return Err(Error::invalid("INVALID_MULTISIG_PAYMENT"));
    }
    Ok(format!("{PAYMENT_PREFIX}{encoded}"))
}

#[uniffi::export]
pub fn bsv_multisig_payment_request_from_script(name: String, output_script: String, amount: String) -> Result<BsvHomeBsvMultisigPaymentRequest, Error> {
    let configuration = parse_locking_script(&output_script)?;
    let descriptor = policy_descriptor(&Policy {
        threshold: u32::from(configuration.threshold),
        public_keys: configuration.public_keys,
    })?;
    let request = BsvHomeBsvMultisigPaymentRequest {
        name,
        threshold: descriptor.policy.threshold,
        public_keys: descriptor.policy.public_keys,
        fingerprint: descriptor.configuration_hash,
        amount,
    };
    request.validate()?;
    Ok(request)
}

impl BsvHomeBsvMultisigPaymentRequest {
    pub(super) fn validate(&self) -> Result<(), Error> {
        let unsafe_name = UNSAFE_NAME.as_ref().map_err(Clone::clone)?;
        if self.name.trim().is_empty() || self.name.encode_utf16().count() > MAX_NAME_LENGTH || unsafe_name.is_match(&self.name) {
            return Err(Error::invalid("INVALID_MULTISIG_PAYMENT"));
        }
        let policy = Policy {
            threshold: self.threshold,
            public_keys: self.public_keys.clone(),
        };
        let descriptor = policy_descriptor(&policy)?;
        if self.public_keys != descriptor.policy.public_keys || self.fingerprint != descriptor.configuration_hash {
            return Err(Error::invalid("INVALID_MULTISIG_PAYMENT"));
        }
        if !self.amount.is_empty() {
            if self.amount.len() > MAX_AMOUNT_LENGTH || self.amount.starts_with('0') && self.amount != "0" && !self.amount.starts_with("0.") {
                return Err(Error::invalid("INVALID_AMOUNT"));
            }
            rules::amount_bsv(&self.amount)?;
        }
        Ok(())
    }
}

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    pub fn payment_request(&self, wallet_id: String, amount: String) -> Result<BsvHomeBsvMultisigPaymentRequest, Error> {
        self.workflow_checkpoint()?;
        if !self.get_wallet(wallet_id.clone())?.can_transact {
            return Err(Error::ConfigurationNotConfirmed);
        }
        let wallet = self.remote_wallet(&wallet_id)?;
        let descriptor = policy_descriptor(&rules::wallet_policy(&wallet)?)?;
        let request = BsvHomeBsvMultisigPaymentRequest {
            name: wallet.name,
            threshold: descriptor.policy.threshold,
            public_keys: descriptor.policy.public_keys,
            fingerprint: descriptor.configuration_hash,
            amount,
        };
        request.validate()?;
        self.workflow_checkpoint()?;
        Ok(request)
    }
}

#[uniffi::export]
pub fn bsv_multisig_scan_recipient(value: String) -> Result<BsvHomePaymentRecipient, Error> {
    let BsvHomePayment::Request { request } = PaymentURLDecoder::decode(&value).map_err(|_| Error::invalid("INVALID_RECIPIENT"))? else {
        return Err(Error::invalid("INVALID_RECIPIENT"));
    };
    scan_recipient_request(request)
}

fn scan_recipient_request(request: BsvHomePaymentRequest) -> Result<BsvHomePaymentRecipient, Error> {
    if request.memo.as_ref().is_some_and(|memo| !memo.is_empty()) || request.references.as_ref().is_some_and(|references| !references.is_empty()) {
        return Err(Error::invalid("INVALID_RECIPIENT"));
    }
    let has_amount = request.amount.is_some();
    let asset_id = AssetId::from_chain(Chain::BitcoinSv);
    let destination = payment_transfer_destination(
        &request,
        BsvHomePaymentWalletAsset {
            asset_id: asset_id.clone(),
            decimals: 8,
        },
    );
    let (address, amount) = match destination {
        BsvHomePaymentDestination::Confirm { transfer } => {
            let amount = BigNumberFormatter::big_decimal_value(&transfer.value.to_string(), 8)
                .map_err(|_| Error::invalid("INVALID_AMOUNT"))?
                .normalized()
                .to_plain_string();
            (transfer.address, Some(amount))
        }
        BsvHomePaymentDestination::Recipient {
            asset_id: scanned_asset_id,
            payment,
        } if scanned_asset_id == asset_id => (payment.recipient.address, payment.amount),
        BsvHomePaymentDestination::Recipient { .. } | BsvHomePaymentDestination::SelectAsset { .. } | BsvHomePaymentDestination::Unsupported => {
            return Err(Error::invalid("INVALID_RECIPIENT"));
        }
    };
    BsvAddress::from_str(&address).map_err(|_| Error::invalid("INVALID_RECIPIENT"))?;
    if let Some(amount) = &amount {
        rules::amount_bsv(amount)?;
    } else if has_amount {
        return Err(Error::invalid("INVALID_AMOUNT"));
    }
    Ok(BsvHomePaymentRecipient {
        recipient: BsvHomeRecipient::address(address),
        amount,
    })
}
