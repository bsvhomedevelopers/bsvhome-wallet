#[derive(Clone, Copy, Debug, PartialEq, uniffi::Enum)]
pub enum BsvHomeMemoWarning {
    NotSupported,
    DestinationTag,
    Memo,
}
