mod explorer;
mod get_swap;
mod quote;

use bsv_home_client::Client;
use std::fmt::Debug;

#[derive(Clone, Debug)]
pub struct MayanClient<C>
where
    C: Client + Clone + Send + Sync + Debug + 'static,
{
    pub(super) client: C,
}

impl<C> MayanClient<C>
where
    C: Client + Clone + Send + Sync + Debug + 'static,
{
    pub fn new(client: C) -> Self {
        Self { client }
    }
}
