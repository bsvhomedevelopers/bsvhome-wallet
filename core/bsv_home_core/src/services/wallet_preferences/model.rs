#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeDiscoveryStep {
    Assets,
    Transactions,
    Nfts,
}
