use primitives::block_explorer::ExplorerInput;

pub type BsvHomeExplorerInput = ExplorerInput;

#[uniffi::remote(Record)]
pub struct BsvHomeExplorerInput {
    pub hash: String,
    pub recipient: Option<String>,
    pub memo: Option<String>,
}
