use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use super::{BsvHomeMpcDeviceProof, BsvHomeMpcError, BsvHomeMpcParticipant};

#[derive(Default)]
pub(super) struct MemoryMpcParticipant {
    pub replies: Mutex<VecDeque<Value>>,
    pub requests: Mutex<Vec<Value>>,
    pub closed: Mutex<Vec<String>>,
    pub close_failures: Mutex<VecDeque<BsvHomeMpcError>>,
}

#[async_trait]
impl BsvHomeMpcParticipant for MemoryMpcParticipant {
    async fn exchange(&self, _wallet_id: String, message_json: String) -> Result<String, BsvHomeMpcError> {
        self.requests.lock().unwrap().push(serde_json::from_str(&message_json).unwrap());
        Ok(serde_json::to_string(&self.replies.lock().unwrap().pop_front().unwrap()).unwrap())
    }
    async fn close(&self, wallet_id: String) -> Result<(), BsvHomeMpcError> {
        self.closed.lock().unwrap().push(wallet_id);
        if let Some(error) = self.close_failures.lock().unwrap().pop_front() {
            return Err(error);
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct MemoryMpcDeviceProof {
    pub messages: Mutex<Vec<String>>,
}

#[async_trait]
impl BsvHomeMpcDeviceProof for MemoryMpcDeviceProof {
    async fn public_key(&self) -> Result<String, BsvHomeMpcError> {
        Ok("test-public-device-key".into())
    }
    async fn sign_message(&self, message: String) -> Result<String, BsvHomeMpcError> {
        self.messages.lock().unwrap().push(message);
        Ok("test-device-signature".into())
    }
}
