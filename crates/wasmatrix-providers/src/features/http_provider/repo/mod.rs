use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use wasmatrix_core::{CoreError, Result};

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Value>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub trait HttpProviderRepository: Send + Sync {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse>;
}

pub struct ReqwestHttpProviderRepository {
    client: Client,
}

impl ReqwestHttpProviderRepository {
    pub fn new() -> Result<Self> {
        let client = Client::builder().build().map_err(|e| {
            CoreError::WasmRuntimeError(format!("failed to build http client: {e}"))
        })?;
        Ok(Self { client })
    }
}

impl HttpProviderRepository for ReqwestHttpProviderRepository {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse> {
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|e| {
            CoreError::InvalidCapabilityAssignment(format!("invalid HTTP method: {e}"))
        })?;

        let mut headers = HeaderMap::new();
        for (key, value) in &request.headers {
            let key = HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                CoreError::InvalidCapabilityAssignment(format!("invalid header name '{key}': {e}"))
            })?;
            let value = HeaderValue::from_str(value).map_err(|e| {
                CoreError::InvalidCapabilityAssignment(format!(
                    "invalid header value for '{key}': {e}"
                ))
            })?;
            headers.insert(key, value);
        }

        let mut builder = self.client.request(method, &request.url).headers(headers);
        if let Some(timeout) = request.timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout));
        }
        if let Some(body) = &request.body {
            builder = builder.json(body);
        }

        let response = builder.send().map_err(|e| {
            CoreError::WasmRuntimeError(format!("failed to execute HTTP request: {e}"))
        })?;
        let status = response.status().as_u16();

        let mut response_headers = HashMap::new();
        for (k, v) in response.headers() {
            response_headers.insert(k.to_string(), v.to_str().unwrap_or_default().to_string());
        }

        let body = response.text().map_err(|e| {
            CoreError::SerializationError(format!("failed to read HTTP response body: {e}"))
        })?;

        Ok(HttpResponse {
            status,
            headers: response_headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_execute_rejects_invalid_http_method_before_send() {
        let repo = ReqwestHttpProviderRepository::new().unwrap();
        let req = HttpRequest {
            method: "???".to_string(),
            url: "https://example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_ms: Some(1_000),
        };

        let err = repo.execute(&req).unwrap_err();
        assert!(matches!(err, CoreError::InvalidCapabilityAssignment(_)));
    }
}
