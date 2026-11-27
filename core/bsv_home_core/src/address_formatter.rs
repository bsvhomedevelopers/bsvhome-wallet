use primitives::{AddressFormatStyle, AddressFormatter, Chain};

pub type BsvHomeAddressFormatStyle = AddressFormatStyle;

#[uniffi::remote(Enum)]
pub enum BsvHomeAddressFormatStyle {
    Short,
    Full,
    Extra { extra: u32 },
}

pub fn format_address(address: &str, chain: Option<Chain>, style: BsvHomeAddressFormatStyle) -> String {
    AddressFormatter::format(address, chain, style)
}

#[derive(Default, uniffi::Object)]
pub struct BsvHomeAddressService {}

#[uniffi::export]
impl BsvHomeAddressService {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {}
    }

    pub fn format(&self, address: String, chain: Option<Chain>, style: BsvHomeAddressFormatStyle) -> String {
        format_address(&address, chain, style)
    }
}
