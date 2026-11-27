use bsv_home_client::Target;
use primitives::Chain;

#[derive(Clone, Copy, Debug)]
pub enum BsvHomeStaticApiTarget {
    GetValidators { chain: Chain },
}

impl Target for BsvHomeStaticApiTarget {
    fn path(&self) -> String {
        match self {
            Self::GetValidators { chain } => format!("/blockchains/{}/validators.json", chain.as_ref()),
        }
    }
}
