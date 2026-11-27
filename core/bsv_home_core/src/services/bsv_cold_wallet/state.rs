use std::fmt::Display;

use bsv_home_bsv::cold_wallet::{Pairing, Request, Review, Signed, decode_json};
use primitives::{BsvSpendMetadata, GasPriceType};
use serde::{Deserialize, Serialize};

use super::{BsvHomeBsvColdWalletPending, BsvHomeBsvColdWalletReview};
use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
    pub version: u32,
    pub pairing: String,
    pub pending: Option<Pending>,
    #[serde(default)]
    pub next: Option<Pending>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reservation {
    pub id: String,
    pub spend: BsvSpendMetadata,
    pub gas_price: GasPriceType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pending {
    pub request: String,
    pub review: String,
    pub signed: Option<String>,
    pub broadcasted: bool,
    #[serde(skip)]
    pub persistence_error: Option<String>,
    pub reservation: Option<Reservation>,
}

impl State {
    pub fn new(pairing: String) -> Result<Self, BsvHomeServiceError> {
        let value = Self {
            version: 1,
            pairing,
            pending: None,
            next: None,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<Pairing, BsvHomeServiceError> {
        let pairing: Pairing = decode_json(&self.pairing).map_err(invalid)?;
        pairing.validate().map_err(invalid)?;
        if self.version != 1 {
            return Err(invalid("Unsupported cold wallet state version"));
        }
        if let Some(pending) = &self.pending {
            pending.validate(&pairing)?;
        }
        if let Some(next) = &self.next {
            if !self.pending.as_ref().is_some_and(|pending| pending.signed.is_some()) || next.signed.is_some() || next.broadcasted || next.reservation.is_some() {
                return Err(invalid("Cold wallet next request has no retained previous signature"));
            }
            next.validate(&pairing)?;
        }
        Ok(pairing)
    }
}

impl Pending {
    pub fn validate(&self, pairing: &Pairing) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        let request: Request = decode_json(&self.request).map_err(invalid)?;
        let review: Review = decode_json(&self.review).map_err(invalid)?;
        if request.review(pairing).map_err(invalid)? != review {
            return Err(invalid("Cold wallet saved review differs from request"));
        }
        let transaction_id = self
            .signed
            .as_ref()
            .map(|json| {
                let signed: Signed = decode_json(json).map_err(invalid)?;
                request.transaction_id(&review, &signed).map_err(invalid)
            })
            .transpose()?;
        if self.broadcasted && transaction_id.is_none() {
            return Err(invalid("Cold wallet broadcast state has no verified signed transaction"));
        }
        if let Some(reservation) = &self.reservation {
            let outpoints = request.outpoints().map_err(invalid)?;
            if reservation.id.is_empty()
                || outpoints.len() != reservation.spend.inputs.len()
                || outpoints
                    .iter()
                    .zip(&reservation.spend.inputs)
                    .any(|((hash, vout), input)| hash != &input.transaction_id || *vout != input.vout)
            {
                return Err(invalid("Cold wallet saved reservation differs from request"));
            }
        }
        Ok(BsvHomeBsvColdWalletPending {
            request_json: self.request.clone(),
            review_json: self.review.clone(),
            review: BsvHomeBsvColdWalletReview {
                commitment: review.commitment,
                wallet_id: review.wallet_id,
                recipient: review.recipient,
                amount: review.amount,
                fee: review.fee,
                total_debit: review.amount.checked_add(review.fee).ok_or_else(|| invalid("Cold wallet total overflow"))?,
                change: review.change,
                change_address: review.change_address,
                input_count: review.input_count,
            },
            signed_json: self.signed.clone(),
            transaction_id,
            broadcasted: self.broadcasted,
            persistence_error: self.persistence_error.clone(),
        })
    }
}

pub(super) fn invalid(error: impl Display) -> BsvHomeServiceError {
    BsvHomeServiceError::InvalidInput { msg: error.to_string() }
}
