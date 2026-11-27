use std::error::Error;

use bsv_home_encoding::protobuf::{decode_grpc_message, encode_grpc_message};
use bsv_home_jsonrpc::grpc::{GrpcTransport, ReqwestGrpcTransport};

use super::model::{LookupNameRequest, LookupNameResponse};

const LOOKUP_NAME_PATH: &str = "/sui.rpc.v2.NameService/LookupName";

pub struct SuinsClient {
    url: String,
    transport: ReqwestGrpcTransport,
}

impl SuinsClient {
    pub fn new(url: String) -> Self {
        Self {
            url,
            transport: ReqwestGrpcTransport::new(),
        }
    }

    pub async fn lookup_name(&self, name: &str) -> Result<LookupNameResponse, Box<dyn Error + Send + Sync>> {
        let request = LookupNameRequest { name: Some(name.to_string()) };
        let response = self.transport.unary(&self.url, LOOKUP_NAME_PATH, encode_grpc_message(&request)).await?;
        decode_grpc_message(&response)
    }
}
