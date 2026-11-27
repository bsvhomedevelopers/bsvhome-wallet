use serde_json::Value;
use url::form_urlencoded::Serializer;

use crate::alien::AlienHttpMethod;

use super::BsvHomePortalAuthError;

const MAX_REQUEST_BYTES: usize = 2_100_000;
const MAX_CURSOR_BYTES: usize = 2048;

pub(crate) enum BsvMultisigTarget {
    Wallets,
    Wallet { wallet_id: String },
    Utxos { wallet_id: String },
    ProposalPage { wallet_id: String, offset: u32, limit: u32 },
    Receipts { wallet_id: String },
    ReceiptPage { wallet_id: String, cursor: String },
    PaymailSettings { wallet_id: String },
    LegacyPaymail { wallet_id: String },
    UpdatePaymailSettings { wallet_id: String, body: Value },
    Create { body: Value },
    Invites { wallet_id: String, body: Value },
    PreviewInvite { body: Value },
    Join { body: Value },
    Confirm { wallet_id: String, body: Value },
    Prepare { wallet_id: String, body: Value },
    Signatures { proposal_id: String, body: Value },
    Withdraw { proposal_id: String, body: Value },
    Refresh { proposal_id: String },
    Broadcast { proposal_id: String, body: Value },
}

impl BsvMultisigTarget {
    pub(super) fn request(self) -> Result<(String, AlienHttpMethod, Option<Vec<u8>>), BsvHomePortalAuthError> {
        let (path, body) = match self {
            Self::Wallets => ("/wallets".into(), None),
            Self::Wallet { wallet_id } => (format!("/wallets/{}", identifier(&wallet_id)?), None),
            Self::Utxos { wallet_id } => (format!("/wallets/{}/utxos", identifier(&wallet_id)?), None),
            Self::ProposalPage { wallet_id, offset, limit } => {
                if !(1..=200).contains(&limit) {
                    return Err(BsvHomePortalAuthError::InvalidSession);
                }
                (format!("/wallets/{}/proposal-page?offset={offset}&limit={limit}", identifier(&wallet_id)?), None)
            }
            Self::Receipts { wallet_id } => (format!("/wallets/{}/receipts", identifier(&wallet_id)?), None),
            Self::ReceiptPage { wallet_id, cursor } => {
                if cursor.len() > MAX_CURSOR_BYTES {
                    return Err(BsvHomePortalAuthError::InvalidSession);
                }
                let mut query = Serializer::new(String::new());
                query.append_pair("cursor", &cursor);
                (format!("/wallets/{}/receipt-page?{}", identifier(&wallet_id)?, query.finish()), None)
            }
            Self::PaymailSettings { wallet_id } => (format!("/wallets/{}/paymail/settings", identifier(&wallet_id)?), None),
            Self::LegacyPaymail { wallet_id } => (format!("/wallets/{}/paymail", identifier(&wallet_id)?), None),
            Self::UpdatePaymailSettings { wallet_id, body } => (format!("/wallets/{}/paymail/settings", identifier(&wallet_id)?), Some(body)),
            Self::Create { body } => ("/wallets".into(), Some(body)),
            Self::Invites { wallet_id, body } => (format!("/wallets/{}/invites", identifier(&wallet_id)?), Some(body)),
            Self::PreviewInvite { body } => ("/invites/preview".into(), Some(body)),
            Self::Join { body } => ("/join".into(), Some(body)),
            Self::Confirm { wallet_id, body } => (format!("/wallets/{}/confirm", identifier(&wallet_id)?), Some(body)),
            Self::Prepare { wallet_id, body } => (format!("/wallets/{}/proposals/prepare", identifier(&wallet_id)?), Some(body)),
            Self::Signatures { proposal_id, body } => (format!("/proposals/{}/signatures", identifier(&proposal_id)?), Some(body)),
            Self::Withdraw { proposal_id, body } => (format!("/proposals/{}/withdraw", identifier(&proposal_id)?), Some(body)),
            Self::Refresh { proposal_id } => (format!("/proposals/{}/refresh", identifier(&proposal_id)?), Some(Value::Object(Default::default()))),
            Self::Broadcast { proposal_id, body } => (format!("/proposals/{}/broadcast", identifier(&proposal_id)?), Some(body)),
        };
        let method = if body.is_some() { AlienHttpMethod::Post } else { AlienHttpMethod::Get };
        let body = body
            .map(|body| {
                if !body.is_object() {
                    return Err(BsvHomePortalAuthError::InvalidSession);
                }
                let bytes = serde_json::to_vec(&body).map_err(|_| BsvHomePortalAuthError::InvalidSession)?;
                if bytes.len() > MAX_REQUEST_BYTES {
                    return Err(BsvHomePortalAuthError::Api {
                        code: "MULTISIG_REQUEST_TOO_LARGE".into(),
                        msg: "multisig request exceeds the size limit".into(),
                    });
                }
                Ok(bytes)
            })
            .transpose()?;
        Ok((format!("/auth/multisig{path}"), method, body))
    }
}

fn identifier(value: &str) -> Result<&str, BsvHomePortalAuthError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
        return Err(BsvHomePortalAuthError::InvalidSession);
    }
    Ok(value)
}
