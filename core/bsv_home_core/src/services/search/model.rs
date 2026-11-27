#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSearchScope {
    All,
    List { id: String },
}
