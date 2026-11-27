use crate::rpc::client::ProviderError;
use num_bigint::BigUint;
use primitives::{Transaction, TransactionDirection};
use std::collections::HashSet;

pub fn finalize_wallet(mut transaction: Transaction, addresses: &[String]) -> Result<Transaction, ProviderError> {
    let owners = addresses.iter().map(String::as_str).collect::<HashSet<_>>();
    let outputs = transaction.utxo_outputs.as_ref().ok_or("BSV transaction outputs missing")?;
    let inputs = transaction.utxo_inputs.as_deref().unwrap_or(&[]);
    let owned_inputs = inputs.iter().filter(|input| owners.contains(input.address.as_str())).collect::<Vec<_>>();
    let owned_outputs = outputs.iter().filter(|output| owners.contains(output.address.as_str())).collect::<Vec<_>>();
    if owned_inputs.is_empty() && owned_outputs.is_empty() {
        return Err("BSV wallet history has no owned inputs or outputs".into());
    }
    let input_value = owned_inputs.iter().map(|input| &input.value).sum::<BigUint>();
    let output_value = owned_outputs.iter().map(|output| &output.value).sum::<BigUint>();
    if let Some(input) = owned_inputs.first() {
        transaction.from = input.address.clone();
        let external = outputs.iter().filter(|output| !owners.contains(output.address.as_str())).collect::<Vec<_>>();
        if let Some(recipient) = external.first() {
            transaction.direction = TransactionDirection::Outgoing;
            transaction.to = recipient.address.clone();
            transaction.value = external.iter().map(|output| &output.value).sum();
        } else {
            transaction.direction = TransactionDirection::SelfTransfer;
            transaction.to = owned_outputs.first().ok_or("BSV internal transfer has no owned output")?.address.clone();
            transaction.value = if input_value >= output_value {
                input_value - output_value
            } else {
                output_value - input_value
            };
        }
    } else {
        transaction.direction = TransactionDirection::Incoming;
        transaction.to = owned_outputs.first().ok_or("BSV incoming transfer has no owned output")?.address.clone();
        transaction.value = output_value;
    }
    Ok(transaction)
}

pub fn finalize(mut transaction: Transaction, address: &str) -> Result<Transaction, ProviderError> {
    if transaction.input_addresses().is_empty() {
        let outputs = transaction.utxo_outputs.as_ref().ok_or("BSV transaction outputs missing")?;
        let owned: Vec<_> = outputs.iter().filter(|output| output.address == address).collect();
        if owned.is_empty() {
            return Err("BSV incoming history has no owned output".into());
        }
        transaction.to = address.to_string();
        transaction.value = owned.iter().map(|output| &output.value).sum::<BigUint>();
        transaction.direction = TransactionDirection::Incoming;
        return Ok(transaction);
    }
    Ok(transaction.finalize(vec![address.to_string()]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::TransactionUtxoInput;
    #[test]
    fn coinbase_history_counts_only_wallet_outputs() {
        let mut transaction = Transaction::mock();
        transaction.from = "Coinbase".to_string();
        transaction.utxo_outputs = Some(vec![
            TransactionUtxoInput::new("owner".to_string(), 200u64.into()),
            TransactionUtxoInput::new("other".to_string(), 300u64.into()),
        ]);
        let mapped = finalize(transaction, "owner").unwrap();
        assert_eq!(mapped.value, 200u64.into());
        assert_eq!(mapped.to, "owner");
        assert_eq!(mapped.direction, TransactionDirection::Incoming);
        assert!(finalize(mapped, "unknown").is_err());
    }
    #[test]
    fn non_address_inputs_do_not_inflate_incoming_amount() {
        let mut transaction = Transaction::mock();
        transaction.utxo_inputs = Some(Vec::new());
        transaction.utxo_outputs = Some(vec![
            TransactionUtxoInput::new("owner".to_string(), 200u64.into()),
            TransactionUtxoInput::new("other".to_string(), 300u64.into()),
        ]);
        let mapped = finalize(transaction, "owner").unwrap();
        assert_eq!(mapped.value, 200u64.into());
        assert_eq!(mapped.direction, TransactionDirection::Incoming);
    }

    #[test]
    fn test_wallet_projection_counts_change_once_and_internal_net_change() {
        let owners = vec!["receive-0".to_string(), "receive-1".to_string(), "change-0".to_string()];
        let mut transaction = Transaction::mock();
        transaction.utxo_inputs = Some(vec![
            TransactionUtxoInput::new("receive-0".to_string(), 50000u64.into()),
            TransactionUtxoInput::new("receive-1".to_string(), 30000u64.into()),
        ]);
        transaction.utxo_outputs = Some(vec![
            TransactionUtxoInput::new("recipient".to_string(), 60000u64.into()),
            TransactionUtxoInput::new("change-0".to_string(), 19962u64.into()),
        ]);
        let outgoing = finalize_wallet(transaction.clone(), &owners).unwrap();
        assert_eq!(outgoing.direction, TransactionDirection::Outgoing);
        assert_eq!(outgoing.value, 60000u64.into());
        transaction.utxo_outputs.as_mut().unwrap()[0].address = "receive-1".to_string();
        let internal = finalize_wallet(transaction, &owners).unwrap();
        assert_eq!(internal.direction, TransactionDirection::SelfTransfer);
        assert_eq!(internal.value, 38u64.into());

        let mut coinbase = Transaction::mock();
        coinbase.utxo_inputs = Some(Vec::new());
        coinbase.utxo_outputs = Some(vec![
            TransactionUtxoInput::new("receive-0".to_string(), 100u64.into()),
            TransactionUtxoInput::new("receive-1".to_string(), 200u64.into()),
            TransactionUtxoInput::new("other".to_string(), 300u64.into()),
        ]);
        let incoming = finalize_wallet(coinbase, &owners).unwrap();
        assert_eq!(incoming.direction, TransactionDirection::Incoming);
        assert_eq!(incoming.value, 300u64.into());
    }
}
