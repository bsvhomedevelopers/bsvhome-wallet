use bsv_home_jsonrpc::HttpMethod;
use strum::AsRefStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, AsRefStr)]
#[strum(serialize_all = "UPPERCASE")]
pub enum BsvHomeApiMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl From<BsvHomeApiMethod> for HttpMethod {
    fn from(method: BsvHomeApiMethod) -> Self {
        match method {
            BsvHomeApiMethod::Get => HttpMethod::Get,
            BsvHomeApiMethod::Post => HttpMethod::Post,
            BsvHomeApiMethod::Put => HttpMethod::Put,
            BsvHomeApiMethod::Delete => HttpMethod::Delete,
        }
    }
}
