use std::collections::BTreeMap;
use std::fmt;

use bsv_home_bsv::multisig::{Funding, FundingReview, funding_transaction_id, review_funding};
use bsv_home_hash::sha2::sha256;
use primitives::WalletId;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as DeserializeError, MapAccess, Visitor},
};
use url::Url;

use super::model::BsvHomeBsvMultisigFundingStatus;
use super::payment::BsvHomeBsvMultisigPaymentRequest;
use super::{BsvHomeBsvMultisigError as Error, rules};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum BsvHomeBsvMultisigFundingTarget {
    Member { wallet_id: String },
    Public { request: BsvHomeBsvMultisigPaymentRequest },
}

impl BsvHomeBsvMultisigFundingTarget {
    pub(crate) fn validate_funding(&self, funding: &Funding, review: &FundingReview) -> Result<(), Error> {
        if review_funding(funding)? != *review {
            return Err(Error::FundingChanged);
        }
        match self {
            Self::Member { wallet_id } => validate_identifier(wallet_id),
            Self::Public { request } => {
                request.validate()?;
                if request.amount.is_empty()
                    || rules::amount_bsv(&request.amount)? != review.amount
                    || request.threshold != funding.policy.threshold
                    || request.public_keys != funding.policy.public_keys
                    || request.fingerprint != review.configuration_hash
                {
                    return Err(Error::FundingChanged);
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PayerFundingBinding {
    pub scope: String,
    pub operation_id: String,
    pub payer_wallet_id: WalletId,
    pub target: BsvHomeBsvMultisigFundingTarget,
    pub funding_json: String,
    pub review_json: String,
}

impl fmt::Debug for PayerFundingBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PayerFundingBinding")
            .field("scope", &self.scope)
            .field("operation_id", &self.operation_id)
            .field("payer_wallet_id", &self.payer_wallet_id)
            .finish_non_exhaustive()
    }
}

impl PayerFundingBinding {
    pub(crate) fn validate(&self) -> Result<(Funding, FundingReview), Error> {
        validate_identifier(&self.operation_id)?;
        validate_scope(&self.scope, &self.payer_wallet_id, &self.target)?;
        let funding = serde_json::from_str(&self.funding_json)?;
        let review = serde_json::from_str(&self.review_json)?;
        self.target.validate_funding(&funding, &review)?;
        Ok((funding, review))
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PayerFundingCheckpoint {
    pub binding: PayerFundingBinding,
    pub raw: String,
    pub transaction_id: String,
    pub status: BsvHomeBsvMultisigFundingStatus,
    pub error_code: Option<String>,
}

impl fmt::Debug for PayerFundingCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PayerFundingCheckpoint")
            .field("binding", &self.binding)
            .field("transaction_id", &self.transaction_id)
            .field("status", &self.status)
            .field("error_code", &self.error_code)
            .finish_non_exhaustive()
    }
}

impl PayerFundingCheckpoint {
    pub(crate) fn validate(&self) -> Result<(Funding, FundingReview), Error> {
        let (funding, review) = self.binding.validate()?;
        if funding_transaction_id(&funding, &review, &self.raw)? != self.transaction_id {
            return Err(Error::FundingChanged);
        }
        Ok((funding, review))
    }

    pub(crate) fn same_signed_payment(&self, other: &Self) -> bool {
        self.binding == other.binding && self.raw == other.raw && self.transaction_id == other.transaction_id
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LegacyFundingCheckpoint {
    pub scope: String,
    pub wallet_id: String,
    pub payer_wallet_id: String,
    pub funding_json: String,
    pub review_json: String,
    pub raw: String,
    pub transaction_id: String,
    pub status: BsvHomeBsvMultisigFundingStatus,
    pub error_code: Option<String>,
}

impl fmt::Debug for LegacyFundingCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyFundingCheckpoint")
            .field("scope", &self.scope)
            .field("wallet_id", &self.wallet_id)
            .field("payer_wallet_id", &self.payer_wallet_id)
            .field("transaction_id", &self.transaction_id)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl LegacyFundingCheckpoint {
    pub(crate) fn validated_checkpoint(&self) -> Result<PayerFundingCheckpoint, Error> {
        let payer_wallet_id = WalletId::from_id(&self.payer_wallet_id).ok_or(Error::FundingChanged)?;
        let target = BsvHomeBsvMultisigFundingTarget::Member {
            wallet_id: self.wallet_id.clone(),
        };
        validate_scope(&self.scope, &payer_wallet_id, &target)?;
        let funding: Funding = serde_json::from_str(&self.funding_json)?;
        let review: FundingReview = serde_json::from_str(&self.review_json)?;
        target.validate_funding(&funding, &review)?;
        if funding_transaction_id(&funding, &review, &self.raw)? != self.transaction_id {
            return Err(Error::FundingChanged);
        }
        let identifier = serde_json::to_vec(&("bsv-funding-legacy-v1", &self.scope, &self.payer_wallet_id, &self.transaction_id))?;
        let operation_id = format!("legacy-v1-{}", hex::encode(sha256(&identifier)));
        Ok(PayerFundingCheckpoint {
            binding: PayerFundingBinding {
                scope: self.scope.clone(),
                operation_id,
                payer_wallet_id,
                target,
                funding_json: self.funding_json.clone(),
                review_json: self.review_json.clone(),
            },
            raw: self.raw.clone(),
            transaction_id: self.transaction_id.clone(),
            status: self.status.clone(),
            error_code: self.error_code.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) enum FundingCheckpoint {
    Legacy(LegacyFundingCheckpoint),
    Current(PayerFundingCheckpoint),
}

impl FundingCheckpoint {
    pub(crate) fn scope(&self) -> &str {
        match self {
            Self::Legacy(checkpoint) => &checkpoint.scope,
            Self::Current(checkpoint) => &checkpoint.binding.scope,
        }
    }

    pub(crate) fn validated_checkpoint(&self) -> Result<PayerFundingCheckpoint, Error> {
        match self {
            Self::Legacy(checkpoint) => checkpoint.validated_checkpoint(),
            Self::Current(checkpoint) => {
                checkpoint.validate()?;
                Ok(checkpoint.clone())
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FundingCheckpointWire {
    format_version: u32,
    scope: String,
    operation_id: String,
    payer_wallet_id: WalletId,
    target: BsvHomeBsvMultisigFundingTarget,
    funding_json: String,
    review_json: String,
    raw: String,
    transaction_id: String,
    status: BsvHomeBsvMultisigFundingStatus,
    error_code: Option<String>,
}

impl Serialize for FundingCheckpoint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Legacy(checkpoint) => checkpoint.serialize(serializer),
            Self::Current(checkpoint) => FundingCheckpointWire {
                format_version: 1,
                scope: checkpoint.binding.scope.clone(),
                operation_id: checkpoint.binding.operation_id.clone(),
                payer_wallet_id: checkpoint.binding.payer_wallet_id.clone(),
                target: checkpoint.binding.target.clone(),
                funding_json: checkpoint.binding.funding_json.clone(),
                review_json: checkpoint.binding.review_json.clone(),
                raw: checkpoint.raw.clone(),
                transaction_id: checkpoint.transaction_id.clone(),
                status: checkpoint.status.clone(),
                error_code: checkpoint.error_code.clone(),
            }
            .serialize(serializer),
        }
    }
}

#[derive(Default)]
enum FieldPresence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for FieldPresence<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FundingCheckpointInput {
    #[serde(default)]
    format_version: FieldPresence<u32>,
    scope: String,
    #[serde(default)]
    operation_id: FieldPresence<String>,
    payer_wallet_id: String,
    #[serde(default)]
    wallet_id: FieldPresence<String>,
    #[serde(default)]
    target: FieldPresence<BsvHomeBsvMultisigFundingTarget>,
    funding_json: String,
    review_json: String,
    raw: String,
    transaction_id: String,
    status: BsvHomeBsvMultisigFundingStatus,
    error_code: Option<String>,
}

impl<T> FieldPresence<T> {
    fn require_missing<E: DeserializeError>(self) -> Result<(), E> {
        match self {
            Self::Missing => Ok(()),
            Self::Null | Self::Value(_) => Err(E::custom("unexpected funding checkpoint field")),
        }
    }

    fn require_value<E: DeserializeError>(self) -> Result<T, E> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Missing | Self::Null => Err(E::custom("missing funding checkpoint field")),
        }
    }
}

impl<'de> Deserialize<'de> for FundingCheckpoint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = FundingCheckpointInput::deserialize(deserializer)?;
        match (input.wallet_id, input.target) {
            (FieldPresence::Value(wallet_id), FieldPresence::Missing) => {
                input.format_version.require_missing::<D::Error>()?;
                input.operation_id.require_missing::<D::Error>()?;
                Ok(Self::Legacy(LegacyFundingCheckpoint {
                    scope: input.scope,
                    wallet_id,
                    payer_wallet_id: input.payer_wallet_id,
                    funding_json: input.funding_json,
                    review_json: input.review_json,
                    raw: input.raw,
                    transaction_id: input.transaction_id,
                    status: input.status,
                    error_code: input.error_code,
                }))
            }
            (FieldPresence::Missing, FieldPresence::Value(target)) => {
                if input.format_version.require_value::<D::Error>()? != 1 {
                    return Err(D::Error::custom("unsupported funding checkpoint version"));
                }
                let operation_id = input.operation_id.require_value::<D::Error>()?;
                Ok(Self::Current(PayerFundingCheckpoint {
                    binding: PayerFundingBinding {
                        scope: input.scope,
                        operation_id,
                        payer_wallet_id: WalletId::from_id(&input.payer_wallet_id).ok_or_else(|| D::Error::custom("invalid funding payer identity"))?,
                        target,
                        funding_json: input.funding_json,
                        review_json: input.review_json,
                    },
                    raw: input.raw,
                    transaction_id: input.transaction_id,
                    status: input.status,
                    error_code: input.error_code,
                }))
            }
            (FieldPresence::Missing, FieldPresence::Missing | FieldPresence::Null)
            | (FieldPresence::Null, FieldPresence::Missing | FieldPresence::Null | FieldPresence::Value(_))
            | (FieldPresence::Value(_), FieldPresence::Null | FieldPresence::Value(_)) => Err(D::Error::custom("invalid funding checkpoint representation")),
        }
    }
}

pub(crate) fn deserialize_funding_checkpoints<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, FundingCheckpoint>, D::Error> {
    struct FundingCheckpointsVisitor;

    impl<'de> Visitor<'de> for FundingCheckpointsVisitor {
        type Value = BTreeMap<String, FundingCheckpoint>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("distinct funding checkpoint scopes")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut checkpoints = BTreeMap::new();
            while let Some((scope, checkpoint)) = map.next_entry::<String, FundingCheckpoint>()? {
                if checkpoints.insert(scope, checkpoint).is_some() {
                    return Err(M::Error::custom("duplicate funding checkpoint scope"));
                }
            }
            Ok(checkpoints)
        }
    }

    deserializer.deserialize_map(FundingCheckpointsVisitor)
}

fn validate_identifier(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.contains('|') || value.chars().any(char::is_control) {
        return Err(Error::FundingChanged);
    }
    Ok(())
}

fn validate_scope(scope: &str, payer_wallet_id: &WalletId, target: &BsvHomeBsvMultisigFundingTarget) -> Result<(), Error> {
    let parts = scope.split('|').collect::<Vec<_>>();
    let origin = parts.first().ok_or(Error::FundingChanged)?;
    let url = Url::parse(origin).map_err(|_| Error::FundingChanged)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || origin.ends_with('/')
        || url.as_str().trim_end_matches('/') != *origin
    {
        return Err(Error::FundingChanged);
    }
    match target {
        BsvHomeBsvMultisigFundingTarget::Member { wallet_id } => {
            validate_identifier(wallet_id)?;
            if parts.len() != 3 || WalletId::from_id(parts[1]).is_none() || parts[2] != wallet_id {
                return Err(Error::FundingChanged);
            }
        }
        BsvHomeBsvMultisigFundingTarget::Public { .. } => {
            if parts.len() != 2 || WalletId::from_id(parts[1]).as_ref() != Some(payer_wallet_id) {
                return Err(Error::FundingChanged);
            }
        }
    }
    Ok(())
}
