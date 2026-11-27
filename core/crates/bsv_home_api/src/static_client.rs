use bsv_home_client::{Client, ClientError, ClientExt};
use primitives::{Chain, StakeValidator};

use crate::static_target::BsvHomeStaticApiTarget;

#[derive(Debug, Clone)]
pub struct BsvHomeStaticApiClient<C: Client> {
    client: C,
}

impl<C: Client> BsvHomeStaticApiClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn get_validators(&self, chain: Chain) -> Result<Vec<StakeValidator>, ClientError> {
        self.client.get(BsvHomeStaticApiTarget::GetValidators { chain }).await
    }
}
