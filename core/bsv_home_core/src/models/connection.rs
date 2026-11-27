use primitives::{ConnectionComponent, ConnectionStatus};

pub type BsvHomeConnectionStatus = ConnectionStatus;
pub type BsvHomeConnectionComponent = ConnectionComponent;

#[uniffi::export]
pub fn connection_status(unhealthy_components: Vec<BsvHomeConnectionComponent>) -> BsvHomeConnectionStatus {
    ConnectionStatus::from_unhealthy_components(&unhealthy_components)
}
