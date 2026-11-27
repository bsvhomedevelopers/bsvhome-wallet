pub mod client;
pub mod model;
mod target;

#[cfg(test)]
pub(crate) mod testkit;

pub use self::model::*;

use bsv_home_client::ReqwestClient;
pub type JupiterClient = client::JupiterClient<ReqwestClient>;
