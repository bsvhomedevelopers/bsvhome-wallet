use super::docs::DOCS_URL;

#[derive(uniffi::Enum, Clone)]
pub enum RewardsUrl {
    Rewards,
}

const WEBSITE_URL: &str = "https://gemwallet.com";

#[uniffi::export]
impl RewardsUrl {
    pub fn url(&self, locale: Option<String>) -> String {
        let _ = locale;
        let path = match self {
            Self::Rewards => "/rewards/",
        };
        format!("{DOCS_URL}{path}")
    }
}

pub fn get_referral_url(code: &str) -> String {
    format!("{WEBSITE_URL}/join?code={code}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_referral_url() {
        assert_eq!(get_referral_url("BSVHOME123"), "https://gemwallet.com/join?code=BSVHOME123");
    }

    #[test]
    fn test_get_rewards_url() {
        for locale in [None, Some("en"), Some("ru"), Some("zh-Hans"), Some("zh-Hant"), Some("pt-BR")] {
            assert_eq!(RewardsUrl::Rewards.url(locale.map(String::from)), "https://api-dev.hj0252.com/help/rewards/");
        }
    }
}
