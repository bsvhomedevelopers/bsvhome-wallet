use super::rules;
use primitives::{SupportMessage, SupportMessageSender};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSupportChatGroup {
    pub sender: SupportMessageSender,
    pub messages: Vec<SupportMessage>,
}

#[uniffi::export]
pub fn support_chat_groups(messages: Vec<SupportMessage>) -> Vec<BsvHomeSupportChatGroup> {
    rules::chat_groups(messages)
}
