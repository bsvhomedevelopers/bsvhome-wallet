use primitives::contact::ContactAddress;
use primitives::{Chain, Contact};

use super::rules;

#[derive(uniffi::Enum)]
pub enum BsvHomeContactAvatar {
    Empty,
    Image { image_url: String },
    Rendered { image: Vec<u8> },
}

#[derive(uniffi::Record)]
pub struct BsvHomeContactInput {
    pub id: String,
    pub existing: Option<Contact>,
    pub name: String,
    pub description: String,
    pub avatar: BsvHomeContactAvatar,
    pub addresses: Vec<ContactAddress>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeContactScannedAddress {
    pub address: String,
    pub memo: Option<String>,
}

#[derive(uniffi::Record)]
pub struct BsvHomeContactAddressInput {
    pub contact_id: String,
    pub chain: Chain,
    pub address: String,
    pub memo: Option<String>,
    pub replacing_id: Option<String>,
}

#[uniffi::export]
impl BsvHomeContactAddressInput {
    pub fn add_address(&self, addresses: Vec<ContactAddress>) -> Vec<ContactAddress> {
        let address = rules::contact_address(self.contact_id.clone(), self.chain, self.address.clone(), self.memo.clone());
        rules::upsert_address(addresses, address, self.replacing_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_address_replaces_the_selected_address() {
        let existing = ContactAddress {
            id: "old".into(),
            contact_id: "contact".into(),
            address: "0xold".into(),
            chain: Chain::Ethereum,
            memo: None,
        };
        let input = BsvHomeContactAddressInput {
            contact_id: "contact".into(),
            chain: Chain::Ethereum,
            address: "0xnew".into(),
            memo: Some(" note ".into()),
            replacing_id: Some(existing.id.clone()),
        };

        let addresses = input.add_address(vec![existing]);

        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].id, "contact_ethereum_0xnew");
        assert_eq!(addresses[0].memo.as_deref(), Some("note"));
    }
}
