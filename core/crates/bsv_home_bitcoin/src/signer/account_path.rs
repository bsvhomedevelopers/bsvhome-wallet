use std::str::FromStr;

use bitcoin::{
    NetworkKind,
    bip32::{ChildNumber, DerivationPath, Xpub},
};
use primitives::SignerError;

const MAX_DEPTH: usize = 16;
const MAX_PATH_BYTES: usize = 256;

#[derive(Clone, Debug)]
pub struct AccountPath(DerivationPath);

impl AccountPath {
    pub fn parse(input: &str) -> Result<Self, SignerError> {
        let input = input.trim();
        if input.len() > MAX_PATH_BYTES {
            return SignerError::invalid_input_err("account path exceeds limit");
        }
        let segments = input.split('/').collect::<Vec<_>>();
        if segments.first() != Some(&"m") || segments.len() > MAX_DEPTH + 1 {
            return SignerError::invalid_input_err("Invalid account root or depth");
        }
        let children = segments
            .iter()
            .skip(1)
            .map(|segment| {
                let (number, hardened) = match segment.strip_suffix('\'').or_else(|| segment.strip_suffix('h')).or_else(|| segment.strip_suffix('H')) {
                    Some(number) => (number, true),
                    None => (*segment, false),
                };
                if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
                    return SignerError::invalid_input_err("Invalid account child index");
                }
                let index: u32 = number.parse().map_err(SignerError::from_display)?;
                if index >= 0x8000_0000 {
                    return SignerError::invalid_input_err("account child index must be below 2^31");
                }
                if hardened {
                    ChildNumber::from_hardened_idx(index).map_err(SignerError::from_display)
                } else {
                    ChildNumber::from_normal_idx(index).map_err(SignerError::from_display)
                }
            })
            .collect::<Result<Vec<_>, SignerError>>()?;
        Ok(Self(DerivationPath::from(children)))
    }

    pub fn canonical(&self) -> String {
        format!("m{}", self.0.as_ref().iter().map(|child| format!("/{child}")).collect::<String>())
    }
    pub fn derivation(&self) -> &DerivationPath {
        &self.0
    }

    pub fn validate_xpub(&self, value: &str) -> Result<Xpub, SignerError> {
        let xpub = parse_xpub(value)?;
        let children = self.0.as_ref();
        let child = children.last().copied().unwrap_or(ChildNumber::Normal { index: 0 });
        if xpub.depth as usize != children.len() || xpub.child_number != child {
            return SignerError::invalid_input_err("xPub depth or final child does not match account path");
        }
        Ok(xpub)
    }
}

pub fn parse_xpub(value: &str) -> Result<Xpub, SignerError> {
    if value.len() > 120 {
        return SignerError::invalid_input_err("xPub exceeds limit");
    }
    let xpub = Xpub::from_str(value).map_err(SignerError::from_display)?;
    if xpub.network != NetworkKind::Main
        || xpub.depth as usize > MAX_DEPTH
        || (xpub.depth == 0 && (xpub.parent_fingerprint != Default::default() || xpub.child_number != (ChildNumber::Normal { index: 0 })))
    {
        return SignerError::invalid_input_err("Invalid mainnet account xPub");
    }
    Ok(xpub)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_account_path_bounds_and_canonical_indices() {
        assert_eq!(AccountPath::parse(" m/044h/236H/02'/7 ").unwrap().canonical(), "m/44'/236'/2'/7");
        assert_eq!(AccountPath::parse("m").unwrap().canonical(), "m");
        assert_eq!(AccountPath::parse(&format!("m{}", "/2147483647'".repeat(16))).unwrap().derivation().as_ref().len(), 16);
        for path in ["M", "m/", "m//0", "m/-1", "m/+1", "m/2147483648", "m/2147483648'", "m/1.0", "m/0hh", "m/0/'"] {
            assert!(AccountPath::parse(path).is_err());
        }
        assert!(AccountPath::parse(&format!("m{}", "/0".repeat(17))).is_err());
    }
}
