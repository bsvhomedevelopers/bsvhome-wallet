use super::{BsvHomeLocalizedText, BsvHomeLocalizer};

#[derive(Default)]
pub struct EnglishLocalizer;

impl BsvHomeLocalizer for EnglishLocalizer {
    fn text(&self, text: BsvHomeLocalizedText) -> String {
        match text {
            BsvHomeLocalizedText::WalletDefaultName { index } => format!("Wallet #{index}"),
            BsvHomeLocalizedText::WalletDefaultNameChain { chain, index } => format!("{} Wallet #{index}", chain.as_ref()),
        }
    }
}
