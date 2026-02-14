use crate::features::http_provider::repo::{HttpProviderRepository, HttpRequest};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use wasmatrix_core::{CapabilityAssignment, CoreError, Result};

pub struct HttpProviderService {
    repo: Arc<dyn HttpProviderRepository>,
}

impl HttpProviderService {
    pub fn new(repo: Arc<dyn HttpProviderRepository>) -> Self {
        Self { repo }
    }

    pub fn execute_request(
        &self,
        assignment: &CapabilityAssignment,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Option<Value>,
        timeout_ms: Option<u64>,
    ) -> Result<Value> {
        self.validate_permission(assignment, url)?;

        let req = HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers,
            body,
            timeout_ms,
        };
        let res = self.repo.execute(&req)?;

        Ok(serde_json::json!({
            "status": res.status,
            "headers": res.headers,
            "body": res.body
        }))
    }

    fn validate_permission(&self, assignment: &CapabilityAssignment, url: &str) -> Result<()> {
        if !assignment.has_permission("http:request") {
            return Err(CoreError::InvalidCapabilityAssignment(
                "Permission denied: missing 'http:request' permission".to_string(),
            ));
        }

        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .ok_or_else(|| {
                CoreError::InvalidCapabilityAssignment(
                    "Invalid URL: unable to extract host".to_string(),
                )
            })?;

        let domain_permission = format!("http:domain:{host}");
        if assignment
            .permissions
            .iter()
            .any(|p| p.starts_with("http:domain:"))
            && !assignment.has_permission(&domain_permission)
        {
            return Err(CoreError::InvalidCapabilityAssignment(format!(
                "Permission denied: missing '{domain_permission}' permission"
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::http_provider::repo::{HttpProviderRepository, HttpResponse};
    use std::sync::RwLock;
    use wasmatrix_core::ProviderType;

    struct StubRepo {
        status: u16,
        body: String,
    }

    impl HttpProviderRepository for StubRepo {
        fn execute(&self, _request: &HttpRequest) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status: self.status,
                headers: HashMap::new(),
                body: self.body.clone(),
            })
        }
    }

    struct RecordingRepo {
        last_request: RwLock<Option<HttpRequest>>,
    }

    impl RecordingRepo {
        fn new() -> Self {
            Self {
                last_request: RwLock::new(None),
            }
        }
    }

    impl HttpProviderRepository for RecordingRepo {
        fn execute(&self, request: &HttpRequest) -> Result<HttpResponse> {
            if let Ok(mut slot) = self.last_request.write() {
                *slot = Some(request.clone());
            }
            Ok(HttpResponse {
                status: 200,
                headers: HashMap::new(),
                body: "ok".to_string(),
            })
        }
    }

    fn assignment(permissions: Vec<&str>) -> CapabilityAssignment {
        CapabilityAssignment::new(
            "i-1".to_string(),
            "http-provider".to_string(),
            ProviderType::Http,
            permissions.into_iter().map(|p| p.to_string()).collect(),
        )
    }

    #[test]
    fn test_validate_permission_requires_http_request() {
        let service = HttpProviderService::new(Arc::new(StubRepo {
            status: 200,
            body: "ok".to_string(),
        }));
        let assignment = assignment(vec!["http:domain:example.com"]);

        let result = service.execute_request(
            &assignment,
            "GET",
            "https://example.com/path",
            HashMap::new(),
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_validate_permission_requires_matching_domain_when_domain_scoped() {
        let service = HttpProviderService::new(Arc::new(StubRepo {
            status: 200,
            body: "ok".to_string(),
        }));
        let assignment = assignment(vec!["http:request", "http:domain:example.com"]);

        let result = service.execute_request(
            &assignment,
            "GET",
            "https://another.example.org/path",
            HashMap::new(),
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_execute_request_success() {
        let service = HttpProviderService::new(Arc::new(StubRepo {
            status: 201,
            body: "created".to_string(),
        }));
        let assignment = assignment(vec!["http:request", "http:domain:example.com"]);

        let result = service
            .execute_request(
                &assignment,
                "POST",
                "https://example.com/items",
                HashMap::new(),
                Some(serde_json::json!({"name":"item1"})),
                Some(1_000),
            )
            .unwrap();

        assert_eq!(result["status"].as_u64(), Some(201));
        assert_eq!(result["body"].as_str(), Some("created"));
    }

    #[test]
    fn test_execute_request_allows_generic_http_request_without_domain_scope() {
        let service = HttpProviderService::new(Arc::new(StubRepo {
            status: 200,
            body: "ok".to_string(),
        }));
        let assignment = assignment(vec!["http:request"]);

        let result = service.execute_request(
            &assignment,
            "GET",
            "https://any-host.example/path",
            HashMap::new(),
            None,
            None,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_execute_request_forwards_http_method() {
        let repo = Arc::new(RecordingRepo::new());
        let service = HttpProviderService::new(repo.clone());
        let assignment = assignment(vec!["http:request"]);

        service
            .execute_request(
                &assignment,
                "DELETE",
                "https://example.com/resource/1",
                HashMap::new(),
                None,
                None,
            )
            .unwrap();

        let method = repo
            .last_request
            .read()
            .ok()
            .and_then(|g| g.clone())
            .map(|r| r.method)
            .unwrap_or_default();
        assert_eq!(method, "DELETE");
    }
}
