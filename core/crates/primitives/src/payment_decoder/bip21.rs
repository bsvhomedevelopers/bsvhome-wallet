use super::amount;
use super::error::{PaymentDecoderError, Result};
use super::query;
use crate::{
    AssetId, Chain,
    payment::{Payment, PaymentAmount, PaymentRequest},
};

const REQUIRED_PARAMETER_PREFIX: &str = "req-";
const QUERY_AMOUNT: &str = "amount";
const QUERY_MEMO: &str = "memo";
const QUERY_LABEL: &str = "label";

pub fn decode(chain: Option<Chain>, path: &str) -> Result<Payment> {
    Ok(Payment::Request {
        request: get_request(chain, path)?,
    })
}

pub fn get_request(chain: Option<Chain>, path: &str) -> Result<PaymentRequest> {
    let (address, query) = path.split_once('?').unwrap_or((path, ""));
    let parameters = query::parameters(query);
    if let Some((required, _)) = parameters.iter().find(|(key, _)| key.starts_with(REQUIRED_PARAMETER_PREFIX)) {
        return Err(PaymentDecoderError::InvalidFormat(format!("Unsupported required parameter: {required}")));
    }

    if address.is_empty() {
        return Err(PaymentDecoderError::MissingField("address".to_string()));
    }

    let amount = query::value(&parameters, QUERY_AMOUNT)
        .map(|value| {
            let amount = match chain {
                Some(chain) => amount::exact(&value, chain),
                None => amount::decimal(&value),
            };
            amount
                .map(|value| PaymentAmount::ExactValue { value })
                .ok_or_else(|| PaymentDecoderError::InvalidFormat("Invalid payment amount".into()))
        })
        .transpose()?;
    Ok(PaymentRequest {
        address: address.to_string(),
        amount,
        memo: query::value(&parameters, QUERY_MEMO),
        label: query::value(&parameters, QUERY_LABEL),
        references: None,
        asset_id: chain.map(AssetId::from_chain),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BITCOIN_ADDRESS: &str = "175tWpb8K1S7NmH4Zx6rewF9WQrcZv245W";

    #[test]
    fn test_decode() {
        let bitcoin = Payment::Request {
            request: PaymentRequest {
                address: BITCOIN_ADDRESS.to_string(),
                asset_id: Some(AssetId::from_chain(Chain::Bitcoin)),
                ..PaymentRequest::mock()
            },
        };

        assert_eq!(decode(Some(Chain::Bitcoin), BITCOIN_ADDRESS).unwrap(), bitcoin);
        assert_eq!(decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?dontexist=")).unwrap(), bitcoin);
        assert_eq!(decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?amount=&memo=&label=")).unwrap(), bitcoin);

        assert_eq!(
            decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?amount=50&label=Luke-Jr&message=Donation%20for%20xyz")).unwrap(),
            Payment::Request {
                request: PaymentRequest {
                    address: BITCOIN_ADDRESS.to_string(),
                    amount: Some(PaymentAmount::ExactValue { value: "50".to_string() }),
                    label: Some("Luke-Jr".to_string()),
                    asset_id: Some(AssetId::from_chain(Chain::Bitcoin)),
                    ..PaymentRequest::mock()
                }
            }
        );
        assert_eq!(
            decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?memo=see%20http%3A%2F%2Fx.com")).unwrap(),
            Payment::Request {
                request: PaymentRequest {
                    address: BITCOIN_ADDRESS.to_string(),
                    memo: Some("see http://x.com".to_string()),
                    asset_id: Some(AssetId::from_chain(Chain::Bitcoin)),
                    ..PaymentRequest::mock()
                }
            }
        );
        assert_eq!(
            decode(Some(Chain::Doge), "DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L?amount=42").unwrap(),
            Payment::Request {
                request: PaymentRequest {
                    address: "DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L".to_string(),
                    amount: Some(PaymentAmount::ExactValue { value: "42".to_string() }),
                    asset_id: Some(AssetId::from_chain(Chain::Doge)),
                    ..PaymentRequest::mock()
                }
            }
        );
        assert_eq!(
            decode(None, &format!("{BITCOIN_ADDRESS}?amount=50.72")).unwrap(),
            Payment::Request {
                request: PaymentRequest {
                    address: BITCOIN_ADDRESS.to_string(),
                    amount: Some(PaymentAmount::ExactValue { value: "50.72".to_string() }),
                    ..PaymentRequest::mock()
                }
            }
        );
    }

    #[test]
    fn test_decode_refuses_what_it_cannot_sign() {
        assert_eq!(
            decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?req-somethingyoudontunderstand=50")),
            Err(PaymentDecoderError::InvalidFormat(
                "Unsupported required parameter: req-somethingyoudontunderstand".to_string()
            ))
        );
        assert_eq!(
            decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?req-dontexist=")),
            Err(PaymentDecoderError::InvalidFormat("Unsupported required parameter: req-dontexist".to_string()))
        );
        assert_eq!(
            decode(Some(Chain::Bitcoin), &format!("{BITCOIN_ADDRESS}?REQ-Pop=initiatingapp%3A")),
            Err(PaymentDecoderError::InvalidFormat("Unsupported required parameter: req-pop".to_string()))
        );
    }
}

#[cfg(test)]
mod amount_integrity_tests {
    use super::*;
    const ADDRESS: &str = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
    #[test]
    fn nonempty_invalid_bip21_amount_is_an_error_for_bsv_btc_and_unchained_requests() {
        for chain in [Some(Chain::BitcoinSv), Some(Chain::Bitcoin), None] {
            for amount in ["-1", "1e-8", "NaN", "not-a-number"] {
                assert!(get_request(chain, &format!("{ADDRESS}?amount={amount}")).is_err());
            }
        }
        for chain in [Some(Chain::BitcoinSv), Some(Chain::Bitcoin)] {
            assert!(get_request(chain, &format!("{ADDRESS}?amount=0.000000001")).is_err());
        }
    }
    #[test]
    fn valid_bip21_exact_amounts_and_absent_or_empty_amount_keep_their_existing_contract() {
        for chain in [Some(Chain::BitcoinSv), Some(Chain::Bitcoin), None] {
            for suffix in ["", "?amount=", "?memo="] {
                assert_eq!(get_request(chain, &format!("{ADDRESS}{suffix}")).unwrap().amount, None);
            }
            for (value, expected) in [("0.00000001", "0.00000001"), ("0.12345678", "0.12345678"), ("1.000000000", "1")] {
                assert_eq!(
                    get_request(chain, &format!("{ADDRESS}?amount={value}")).unwrap().amount,
                    Some(PaymentAmount::ExactValue { value: expected.into() })
                );
            }
        }
    }
}
