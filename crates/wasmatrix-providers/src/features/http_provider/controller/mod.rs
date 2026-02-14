use crate::features::http_provider::service::HttpProviderService;
use serde_json::Value;
use std::collections::HashMap;
use wasmatrix_core::{CapabilityAssignment, CoreError, Result};

pub struct HttpProviderController {
    service: HttpProviderService,
}

impl HttpProviderController {
    pub fn new(service: HttpProviderService) -> Self {
        Self { service }
    }

    pub fn handle_invoke(
        &self,
        instance_id: &str,
        operation: &str,
        params: Value,
    ) -> Result<Value> {
        if operation != "request" {
            return Err(CoreError::InvalidCapabilityAssignment(format!(
                "Unknown HTTP operation: {operation}"
            )));
        }

        let method = params
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CoreError::InvalidCapabilityAssignment("Missing 'method' parameter".to_string())
            })?;

        let url = params.get("url").and_then(Value::as_str).ok_or_else(|| {
            CoreError::InvalidCapabilityAssignment("Missing 'url' parameter".to_string())
        })?;

        let mut headers = HashMap::new();
        if let Some(raw_headers) = params.get("headers").and_then(Value::as_object) {
            for (k, v) in raw_headers {
                let value = v.as_str().ok_or_else(|| {
                    CoreError::InvalidCapabilityAssignment(format!(
                        "Header '{k}' value must be a string"
                    ))
                })?;
                headers.insert(k.clone(), value.to_string());
            }
        }

        let body = params.get("body").cloned();
        let timeout_ms = params.get("timeout_ms").and_then(Value::as_u64);
        let permissions = extract_permissions(&params);
        let assignment = CapabilityAssignment::new(
            instance_id.to_string(),
            "http-provider".to_string(),
            wasmatrix_core::ProviderType::Http,
            permissions,
        );

        self.service
            .execute_request(&assignment, method, url, headers, body, timeout_ms)
    }
}

fn extract_permissions(params: &Value) -> Vec<String> {
    params
        .get("permissions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::http_provider::repo::HttpProviderRepository;
    use crate::features::http_provider::repo::{HttpRequest, HttpResponse};
    use crate::features::http_provider::service::HttpProviderService;
    use std::sync::Arc;

    struct DummyRepo;

    impl HttpProviderRepository for DummyRepo {
        fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status: 200,
                headers: HashMap::new(),
                body: "ok".to_string(),
            })
        }
    }

    #[test]
    fn test_handle_invoke_rejects_unknown_operation() {
        let controller = HttpProviderController::new(HttpProviderService::new(Arc::new(DummyRepo)));
        let result = controller.handle_invoke("i-1", "unknown", serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_invoke_rejects_missing_method() {
        let controller = HttpProviderController::new(HttpProviderService::new(Arc::new(DummyRepo)));
        let params = serde_json::json!({
            "url": "https://example.com",
            "permissions": ["http:request"]
        });
        let result = controller.handle_invoke("i-1", "request", params);
        assert!(result.is_err());
    }
}
