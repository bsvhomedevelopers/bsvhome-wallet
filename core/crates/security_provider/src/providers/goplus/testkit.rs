use super::GoPlusProvider;
use bsv_home_client::testkit::MockClient;

impl GoPlusProvider<MockClient> {
    pub fn mock(client: MockClient) -> Self {
        Self::new(client, "", "", None)
    }
}
