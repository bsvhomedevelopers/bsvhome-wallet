use primitives::Currency;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeCurrencyRow {
    pub currency: Currency,
    pub flag: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeCurrencies {
    pub selected: BsvHomeCurrencyRow,
    pub recommended: Vec<BsvHomeCurrencyRow>,
    pub other: Vec<BsvHomeCurrencyRow>,
}
