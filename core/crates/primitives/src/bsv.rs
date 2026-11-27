use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use crate::SignerError;

const HARDENED_ADDRESS_INDEX: u32 = 1 << 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[typeshare(swift = "Equatable, Hashable, Sendable")]
pub enum BsvAddressBranch {
    Receive,
    Change,
}

impl BsvAddressBranch {
    pub fn index(self) -> u32 {
        match self {
            Self::Receive => 0,
            Self::Change => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[typeshare(swift = "Equatable, Hashable, Sendable")]
pub struct BsvAddressLocator {
    pub branch: BsvAddressBranch,
    pub index: u32,
}

impl BsvAddressLocator {
    pub fn validate(self) -> Result<(), SignerError> {
        if self.index >= HARDENED_ADDRESS_INDEX {
            return SignerError::invalid_input_err("BSV address index must be non-hardened");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bsv_address_locator_non_hardened_boundary() {
        for branch in [BsvAddressBranch::Receive, BsvAddressBranch::Change] {
            assert_eq!(BsvAddressLocator { branch, index: 0 }.validate(), Ok(()));
            assert_eq!(
                BsvAddressLocator {
                    branch,
                    index: HARDENED_ADDRESS_INDEX - 1
                }
                .validate(),
                Ok(())
            );
            for index in [HARDENED_ADDRESS_INDEX, u32::MAX] {
                assert_eq!(
                    BsvAddressLocator { branch, index }.validate(),
                    Err(SignerError::invalid_input("BSV address index must be non-hardened"))
                );
            }
        }
    }
}
