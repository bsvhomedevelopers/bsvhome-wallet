use bitcoin::{Address as BitcoinAddress, CompressedPublicKey, Network, ScriptBuf};
use primitives::{Address, AddressError};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct BsvAddress {
    address: BitcoinAddress,
    script: ScriptBuf,
}

impl BsvAddress {
    pub fn from_public_key(public_key: &[u8]) -> Result<Self, AddressError> {
        let public_key = CompressedPublicKey::from_slice(public_key).map_err(|error| AddressError::new(error.to_string()))?;
        let address = BitcoinAddress::p2pkh(public_key, Network::Bitcoin);
        let script = address.script_pubkey();
        Ok(Self { address, script })
    }
    pub fn script_pubkey(&self) -> ScriptBuf {
        self.script.clone()
    }
}

impl Address for BsvAddress {
    fn try_parse(value: &str) -> Option<Self> {
        let address = BitcoinAddress::from_str(value).ok()?.require_network(Network::Bitcoin).ok()?;
        let script = address.script_pubkey();
        script.is_p2pkh().then_some(Self { address, script })
    }
    fn as_bytes(&self) -> &[u8] {
        &self.script.as_bytes()[3..23]
    }
    fn encode(&self) -> String {
        self.address.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_mainnet_p2pkh() {
        let address = BsvAddress::from_str("1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5").unwrap();
        assert_eq!(address.encode(), "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5");
        for value in [
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
            "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn",
            "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE6",
        ] {
            assert!(!BsvAddress::is_valid(value));
        }
    }
}
