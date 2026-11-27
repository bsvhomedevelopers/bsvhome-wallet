use std::sync::Arc;

use primitives::{AddressName, Asset, Chain, ChainAddress, SimulationPayloadField, SimulationPayloadFieldType, SimulationResult, WalletId};

use crate::keystore::{BsvHomeKeystore, decode_password, keystore_id_for_wallet};
use crate::message::sign_type::{MessageType, SignMessage};
use crate::message::signer::MessageSigner;
use crate::services::confirm::BsvHomeSimulationValue;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::name::BsvHomeNameService;
use crate::services::simulation::BsvHomeSimulationFormatter;
use crate::services::wallet::BsvHomeKeystorePassword;
use primitives::BlockExplorerLink;

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeSignMessagePreview {
    pub message_type: MessageType,
    pub text: String,
    pub primary_fields: Vec<SimulationPayloadField>,
    pub secondary_fields: Vec<SimulationPayloadField>,
    pub has_critical_warning: bool,
    pub header: Option<BsvHomeSimulationValue>,
}

#[derive(uniffi::Object)]
pub struct BsvHomeSignMessageService {
    names: Arc<BsvHomeNameService>,
    explorer: Arc<BsvHomeExplorerService>,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    simulation_formatter: BsvHomeSimulationFormatter,
}

#[uniffi::export]
impl BsvHomeSignMessageService {
    #[uniffi::constructor]
    pub fn new(names: Arc<BsvHomeNameService>, explorer: Arc<BsvHomeExplorerService>, keystore: Arc<BsvHomeKeystore>, password: Arc<dyn BsvHomeKeystorePassword>) -> Self {
        Self {
            names,
            explorer,
            keystore,
            password,
            simulation_formatter: BsvHomeSimulationFormatter::new(),
        }
    }

    pub async fn sign(&self, wallet_id: WalletId, message: SignMessage) -> Result<String, BsvHomeServiceError> {
        let password = decode_password(&self.password.get_password(false)?);
        Ok(MessageSigner::new(message).sign_with_keystore(self.keystore.clone(), keystore_id_for_wallet(wallet_id.id()), password)?)
    }

    pub fn preview(&self, message: SignMessage, simulation: SimulationResult, assets: Vec<Asset>) -> BsvHomeSignMessagePreview {
        let signer = MessageSigner::new(message);
        let has_critical_warning = simulation.has_critical_warning();
        let header = BsvHomeSimulationValue::from_simulation(&simulation, &assets);
        let payload_fields = self.simulation_formatter.payload_fields(simulation.payload, header.is_some());
        let payload = signer.payload_preview(payload_fields).ok().flatten();
        BsvHomeSignMessagePreview {
            message_type: payload.as_ref().map(|preview| preview.message_type).unwrap_or(MessageType::Text),
            text: signer.plain_preview(),
            primary_fields: payload.as_ref().map(|preview| preview.primary.clone()).unwrap_or_default(),
            secondary_fields: payload.map(|preview| preview.secondary).unwrap_or_default(),
            has_critical_warning,
            header,
        }
    }

    pub async fn address_names(&self, chain: Chain, preview: BsvHomeSignMessagePreview) -> Vec<AddressName> {
        let requests: Vec<ChainAddress> = preview
            .primary_fields
            .iter()
            .chain(preview.secondary_fields.iter())
            .filter(|field| field.field_type == SimulationPayloadFieldType::Address)
            .map(|field| ChainAddress::new(chain, field.value.clone()))
            .collect();
        if requests.is_empty() {
            return Vec::new();
        }
        self.names.get_address_names(requests).await.unwrap_or_default()
    }

    pub fn address_url(&self, chain: Chain, address: String) -> BlockExplorerLink {
        self.explorer.get_address_url(chain, address)
    }
}
