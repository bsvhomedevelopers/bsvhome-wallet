use crate::services::portal::BsvHomeTransferPasswordError;

pub(crate) fn validate_new_password(password: &str, confirmation: &str) -> Result<(), BsvHomeTransferPasswordError> {
    if !(8..=128).contains(&password.encode_utf16().count()) || password_is_blank(password) {
        return Err(BsvHomeTransferPasswordError::InvalidPassword);
    }
    if password != confirmation {
        return Err(BsvHomeTransferPasswordError::ConfirmationMismatch);
    }
    Ok(())
}

pub(crate) fn validate_current_password(configured: bool, password: Option<&str>) -> Result<(), BsvHomeTransferPasswordError> {
    match (configured, password) {
        (true, None) => Err(BsvHomeTransferPasswordError::CurrentPasswordRequired),
        (true, Some(password)) => validate_verification_password(password),
        (false, Some(_)) => Err(BsvHomeTransferPasswordError::InvalidPassword),
        (false, None) => Ok(()),
    }
}

pub(crate) fn validate_verification_password(password: &str) -> Result<(), BsvHomeTransferPasswordError> {
    if !(1..=128).contains(&password.encode_utf16().count()) {
        return Err(BsvHomeTransferPasswordError::InvalidPassword);
    }
    Ok(())
}

fn password_is_blank(password: &str) -> bool {
    password
        .chars()
        .all(|character| (character.is_whitespace() && character != '\u{85}') || ('\u{1c}'..='\u{1f}').contains(&character))
}
