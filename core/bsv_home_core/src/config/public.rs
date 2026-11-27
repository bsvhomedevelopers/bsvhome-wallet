use primitives::{BSVHOME_API_HOST, UPSTREAM_ANDROID_PACKAGE_ID};

use super::docs::DOCS_URL;

#[derive(uniffi::Enum, Clone)]
pub enum PublicUrl {
    Website,
    Assets,
    PrivacyPolicy,
    TermsOfService,
    Support,
    CodebaseIos,
    CodebaseAndroid,
    AppStore,
    PlayStore,
    APK,
}

pub const ASSETS_URL: &str = "https://assets.gemwallet.com";
pub const API_URL: &str = "https://api.gemwallet.com";
pub const DEVICE_STREAM_PATH: &str = "/v2/devices/stream";

pub fn device_stream_url() -> String {
    format!("wss://{BSVHOME_API_HOST}{DEVICE_STREAM_PATH}")
}

#[uniffi::export]
impl PublicUrl {
    pub fn url(&self) -> String {
        match self {
            Self::Website => "https://gemwallet.com".to_string(),
            Self::Assets => format!("{DOCS_URL}/assets/"),
            Self::PrivacyPolicy => format!("{DOCS_URL}/privacy/"),
            Self::TermsOfService => format!("{DOCS_URL}/terms/"),
            Self::Support => format!("{DOCS_URL}/support/"),
            Self::CodebaseIos => format!("{DOCS_URL}/open-source/"),
            Self::CodebaseAndroid => "https://github.com/gemwalletcom/gem-android/".to_string(),
            Self::AppStore => format!("{DOCS_URL}/download/"),
            Self::PlayStore => format!("https://play.google.com/store/apps/details?id={UPSTREAM_ANDROID_PACKAGE_ID}"),
            Self::APK => "https://apk.gemwallet.com/gem_wallet_latest.apk".to_string(),
        }
    }
}
