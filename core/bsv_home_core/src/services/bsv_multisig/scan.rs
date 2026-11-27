use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigPaymentRequest, parse_bsv_multisig_invitation, parse_bsv_multisig_payment_request, rules};

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigScanDestination {
    Payment { request: BsvHomeBsvMultisigPaymentRequest },
    Invitation { token: String },
}

#[uniffi::export]
pub fn bsv_multisig_scan_destination(value: String) -> Result<Option<BsvHomeBsvMultisigScanDestination>, Error> {
    let value = value.trim();
    if value.starts_with("bsvhome://multisig/pay?") {
        return Ok(Some(BsvHomeBsvMultisigScanDestination::Payment {
            request: parse_bsv_multisig_payment_request(value.into())?,
        }));
    }
    if value.starts_with("bsvhome://multisig/join?") || rules::validate_invitation_token(value).is_ok() {
        return Ok(Some(BsvHomeBsvMultisigScanDestination::Invitation {
            token: parse_bsv_multisig_invitation(value.into())?,
        }));
    }
    Ok(None)
}
