use std::fmt;

use zeroize::Zeroizing;

use crate::BsvHomeCoreError;

#[derive(Clone)]
pub struct BsvHomeDeviceKeyPair {
    pub private_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl fmt::Debug for BsvHomeDeviceKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BsvHomeDeviceKeyPair")
            .field("private_key", &"<redacted>")
            .field("public_key", &self.public_key)
            .finish()
    }
}

pub fn generate_device_key_pair() -> BsvHomeDeviceKeyPair {
    let seed = Zeroizing::new(bsv_home_crypto::random::bytes::<32>().expect("OS RNG must provide 32 bytes for the device key"));
    let public_key = bsv_home_auth::device_public_key(seed.as_slice()).expect("32 bytes is a valid Ed25519 seed");
    BsvHomeDeviceKeyPair {
        private_key: seed.to_vec(),
        public_key: public_key.to_vec(),
    }
}

pub fn device_public_key(private_key: Vec<u8>) -> Result<Vec<u8>, BsvHomeCoreError> {
    let private_key = Zeroizing::new(private_key);
    Ok(bsv_home_auth::device_public_key(&private_key).map_err(BsvHomeCoreError::from)?.to_vec())
}
