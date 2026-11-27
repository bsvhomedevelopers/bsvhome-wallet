pub mod model;
pub mod rules;

pub use model::BsvHomePushNotification;

#[derive(Debug, Default, uniffi::Object)]
pub struct BsvHomePushNotificationService {}

#[uniffi::export]
impl BsvHomePushNotificationService {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {}
    }

    pub fn parse(&self, notification_type: String, data: Option<String>) -> Option<BsvHomePushNotification> {
        rules::notification(&notification_type, data.as_deref())
    }
}
