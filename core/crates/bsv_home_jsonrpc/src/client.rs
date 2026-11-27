use crate::types::{ERROR_CLIENT_ERROR, ERROR_INTERNAL_ERROR, JsonRpcError, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcResult, JsonRpcResults, ToJsonRpcRequest};
use bsv_home_client::{Client, ClientError, ClientExt};
use serde::de::DeserializeOwned;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct JsonRpcClient<C: Client + Clone> {
    client: C,
}

impl From<ClientError> for JsonRpcError {
    fn from(value: ClientError) -> Self {
        if let ClientError::Http { body, .. } = &value
            && let Ok(response) = serde_json::from_slice::<JsonRpcErrorResponse>(body)
        {
            return response.error;
        }
        JsonRpcError {
            code: ERROR_CLIENT_ERROR,
            message: value.to_string(),
            cause: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn http_failures_preserve_valid_jsonrpc_error_details() {
        for status in [400, 500] {
            let body = serde_json::to_vec(&json!({
                "id": 1,
                "error": {"code": -32000, "message": "execution failed", "cause": {"reason": "insufficient balance"}}
            }))
            .unwrap();
            let error = JsonRpcError::from(ClientError::Http { status, body });

            assert_eq!(error.code, -32000);
            assert_eq!(error.message, "execution failed");
            assert_eq!(error.cause, Some(json!({"reason": "insufficient balance"})));
        }
    }

    #[test]
    fn http_failures_with_success_or_unstructured_bodies_remain_client_errors() {
        for body in [br#"{"id":1,"result":"valid-looking"}"#.to_vec(), br#"{}"#.to_vec(), b"unavailable".to_vec()] {
            let error = JsonRpcError::from(ClientError::Http { status: 500, body });

            assert_eq!(error.code, ERROR_CLIENT_ERROR);
            assert_eq!(error.message, "HTTP error: status 500");
        }
    }
}

impl<C: Client + Clone> JsonRpcClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn request<U: DeserializeOwned + Send, T: ToJsonRpcRequest>(&self, request: T) -> Result<U, JsonRpcError> {
        let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        let result: JsonRpcResult<U> = self.send_request(request.to_jsonrpc_request(timestamp)).await?;
        result.take()
    }

    pub async fn batch_request<U: DeserializeOwned + Send, T: ToJsonRpcRequest>(&self, requests: Vec<T>) -> Result<JsonRpcResults<U>, JsonRpcError> {
        let requests: Vec<JsonRpcRequest> = requests.iter().enumerate().map(|(index, request)| request.to_jsonrpc_request(index as u64 + 1)).collect();
        if requests.is_empty() {
            return Ok(Default::default());
        }

        let mut results: Vec<JsonRpcResult<U>> = self.client.post("", &requests).await?;
        results.sort_by_key(JsonRpcResult::id);

        if results.len() != requests.len() || results.iter().enumerate().any(|(index, result)| result.id() != Some(index as u64 + 1)) {
            return Err(JsonRpcError {
                message: "Invalid batch response IDs".into(),
                code: ERROR_INTERNAL_ERROR,
                cause: None,
            });
        }

        Ok(JsonRpcResults(results))
    }

    async fn send_request<T: DeserializeOwned + Send>(&self, request: JsonRpcRequest) -> Result<JsonRpcResult<T>, JsonRpcError> {
        Ok(self.client.post("", &request).await?)
    }
}

#[cfg(feature = "reqwest")]
impl JsonRpcClient<bsv_home_client::ReqwestClient> {
    pub fn new_reqwest(url: String) -> Self {
        use bsv_home_client::ReqwestClient;
        let reqwest_client = bsv_home_client::builder().build().expect("Failed to build reqwest client");
        let client = ReqwestClient::new(url, reqwest_client);
        Self { client }
    }
}
