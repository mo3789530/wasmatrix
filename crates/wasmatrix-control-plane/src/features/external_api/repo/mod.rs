use crate::shared::error::{ControlPlaneError, ControlPlaneResult};
use crate::ControlPlane;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, TimeZone, Utc};
use ring::hmac;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmatrix_core::{CapabilityAssignment, InstanceMetadata, InstanceStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthnType {
    Jwt,
    Mtls,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalApiPrincipal {
    pub subject: String,
    pub authn_type: AuthnType,
    pub roles: Vec<String>,
    pub tenant_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct ExternalApiRepository {
    control_plane: Arc<Mutex<ControlPlane>>,
    jwt_secret: Option<String>,
    jwt_issuer: Option<String>,
    jwt_audience: Option<String>,
    mtls_principals: HashMap<String, ExternalApiPrincipal>,
}

impl ExternalApiRepository {
    pub fn from_env(control_plane: Arc<Mutex<ControlPlane>>) -> Self {
        let mtls_principals = std::env::var("EXTERNAL_API_MTLS_PRINCIPALS")
            .ok()
            .map(|raw| Self::parse_mtls_principals(&raw))
            .unwrap_or_default();

        Self {
            control_plane,
            jwt_secret: std::env::var("EXTERNAL_API_JWT_SECRET").ok(),
            jwt_issuer: std::env::var("EXTERNAL_API_JWT_ISSUER").ok(),
            jwt_audience: std::env::var("EXTERNAL_API_JWT_AUDIENCE").ok(),
            mtls_principals,
        }
    }

    pub fn authenticate_jwt(&self, token: &str) -> ControlPlaneResult<ExternalApiPrincipal> {
        let secret = self.jwt_secret.as_ref().ok_or_else(|| {
            ControlPlaneError::Unauthorized("JWT authentication is not configured".to_string())
        })?;

        let segments: Vec<&str> = token.split('.').collect();
        if segments.len() != 3 {
            return Err(ControlPlaneError::Unauthorized(
                "JWT must contain header, payload, and signature".to_string(),
            ));
        }

        let header = Self::decode_segment(&segments[0], "jwt header")?;
        let payload = Self::decode_segment(&segments[1], "jwt payload")?;
        let signature = URL_SAFE_NO_PAD.decode(segments[2]).map_err(|_| {
            ControlPlaneError::Unauthorized("JWT signature is not valid base64url".to_string())
        })?;

        let alg = header.get("alg").and_then(Value::as_str).ok_or_else(|| {
            ControlPlaneError::Unauthorized("JWT header is missing alg".to_string())
        })?;
        if alg != "HS256" {
            return Err(ControlPlaneError::Unauthorized(format!(
                "Unsupported JWT alg '{alg}'"
            )));
        }

        let signing_input = format!("{}.{}", segments[0], segments[1]);
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        if hmac::verify(&key, signing_input.as_bytes(), &signature).is_err() {
            return Err(ControlPlaneError::Unauthorized(
                "JWT signature verification failed".to_string(),
            ));
        }

        if let Some(expected_issuer) = &self.jwt_issuer {
            let issuer = payload.get("iss").and_then(Value::as_str).ok_or_else(|| {
                ControlPlaneError::Unauthorized("JWT payload is missing iss".to_string())
            })?;
            if issuer != expected_issuer {
                return Err(ControlPlaneError::Unauthorized(
                    "JWT issuer does not match configured issuer".to_string(),
                ));
            }
        }

        if let Some(expected_audience) = &self.jwt_audience {
            let audience_matches = match payload.get("aud") {
                Some(Value::String(aud)) => aud == expected_audience,
                Some(Value::Array(values)) => values
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|aud| aud == expected_audience),
                _ => false,
            };

            if !audience_matches {
                return Err(ControlPlaneError::Unauthorized(
                    "JWT audience does not match configured audience".to_string(),
                ));
            }
        }

        let exp = payload.get("exp").and_then(Value::as_i64);
        let expires_at = exp.and_then(|value| Utc.timestamp_opt(value, 0).single());
        if let Some(expires_at) = expires_at {
            if expires_at <= Utc::now() {
                return Err(ControlPlaneError::Unauthorized(
                    "JWT is expired".to_string(),
                ));
            }
        }

        let subject = payload
            .get("sub")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ControlPlaneError::Unauthorized("JWT payload is missing sub".to_string())
            })?
            .to_string();

        let roles = Self::extract_roles(&payload);
        if roles.is_empty() {
            return Err(ControlPlaneError::Unauthorized(
                "JWT payload must include roles or scope".to_string(),
            ));
        }

        Ok(ExternalApiPrincipal {
            subject,
            authn_type: AuthnType::Jwt,
            roles,
            tenant_id: payload
                .get("tenant_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            expires_at,
        })
    }

    pub fn resolve_mtls_principal(
        &self,
        subject: &str,
    ) -> ControlPlaneResult<Option<ExternalApiPrincipal>> {
        Ok(self.mtls_principals.get(subject).cloned())
    }

    pub fn restore_instance_state(
        &self,
        metadata: InstanceMetadata,
        capabilities: Vec<CapabilityAssignment>,
    ) -> ControlPlaneResult<()> {
        let mut control_plane = self.lock_control_plane()?;
        control_plane.restore_instance_state(metadata, capabilities);
        Ok(())
    }

    pub fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        let control_plane = self.lock_control_plane()?;
        Ok(control_plane
            .list_instances()
            .into_iter()
            .cloned()
            .collect())
    }

    pub fn get_instance(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>> {
        let control_plane = self.lock_control_plane()?;
        Ok(control_plane.get_instance(instance_id).cloned())
    }

    pub fn get_capabilities(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Vec<CapabilityAssignment>> {
        let control_plane = self.lock_control_plane()?;
        Ok(control_plane
            .get_capabilities(instance_id)
            .cloned()
            .unwrap_or_default())
    }

    pub fn set_instance_status(
        &self,
        instance_id: &str,
        status: InstanceStatus,
    ) -> ControlPlaneResult<()> {
        let mut control_plane = self.lock_control_plane()?;
        control_plane
            .update_instance_status(instance_id, status)
            .map_err(|error| ControlPlaneError::InstanceNotFound(error.to_string()))
    }

    pub fn assign_capability(&self, assignment: CapabilityAssignment) -> ControlPlaneResult<()> {
        let mut control_plane = self.lock_control_plane()?;
        control_plane
            .assign_capability(assignment)
            .map_err(|error| ControlPlaneError::ValidationError(error.message))
    }

    pub fn revoke_capability(
        &self,
        instance_id: &str,
        capability_id: &str,
    ) -> ControlPlaneResult<()> {
        let mut control_plane = self.lock_control_plane()?;
        control_plane
            .revoke_capability(instance_id, capability_id)
            .map_err(|error| ControlPlaneError::ValidationError(error.message))
    }

    fn lock_control_plane(&self) -> ControlPlaneResult<std::sync::MutexGuard<'_, ControlPlane>> {
        self.control_plane
            .lock()
            .map_err(|_| ControlPlaneError::StorageError("control plane lock poisoned".to_string()))
    }

    fn decode_segment(segment: &str, label: &str) -> ControlPlaneResult<Value> {
        let bytes = URL_SAFE_NO_PAD.decode(segment).map_err(|_| {
            ControlPlaneError::Unauthorized(format!("{label} is not valid base64url"))
        })?;
        serde_json::from_slice(&bytes)
            .map_err(|_| ControlPlaneError::Unauthorized(format!("{label} is not valid JSON")))
    }

    fn extract_roles(payload: &Value) -> Vec<String> {
        if let Some(roles) = payload.get("roles").and_then(Value::as_array) {
            return roles
                .iter()
                .filter_map(Value::as_str)
                .filter(|role| !role.is_empty())
                .map(ToOwned::to_owned)
                .collect();
        }

        payload
            .get("scope")
            .and_then(Value::as_str)
            .map(|scope| {
                scope
                    .split_whitespace()
                    .filter(|role| !role.is_empty())
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn parse_mtls_principals(raw: &str) -> HashMap<String, ExternalApiPrincipal> {
        let mut principals = HashMap::new();

        for entry in raw.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<&str> = trimmed.split('|').collect();
            if parts.len() < 2 {
                continue;
            }

            let subject = parts[0].trim();
            if subject.is_empty() {
                continue;
            }

            let roles: Vec<String> = parts[1]
                .split('+')
                .filter(|role| !role.trim().is_empty())
                .map(|role| role.trim().to_string())
                .collect();
            if roles.is_empty() {
                continue;
            }

            let tenant_id = parts
                .get(2)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);

            principals.insert(
                subject.to_string(),
                ExternalApiPrincipal {
                    subject: subject.to_string(),
                    authn_type: AuthnType::Mtls,
                    roles,
                    tenant_id,
                    expires_at: None,
                },
            );
        }

        principals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::json;

    fn signed_token(secret: &str, payload: Value) -> String {
        let header = json!({ "alg": "HS256", "typ": "JWT" });
        let encoded_header = URL_SAFE_NO_PAD.encode(header.to_string().as_bytes());
        let encoded_payload = URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
        let signing_input = format!("{encoded_header}.{encoded_payload}");
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let signature = hmac::sign(&key, signing_input.as_bytes());
        let encoded_signature = URL_SAFE_NO_PAD.encode(signature.as_ref());
        format!("{signing_input}.{encoded_signature}")
    }

    #[test]
    fn test_parse_mtls_principals() {
        let principals = ExternalApiRepository::parse_mtls_principals(
            "spiffe://client-a|instance.read+instance.admin|tenant-a",
        );

        let principal = principals.get("spiffe://client-a").unwrap();
        assert_eq!(principal.roles.len(), 2);
        assert_eq!(principal.tenant_id.as_deref(), Some("tenant-a"));
    }

    #[test]
    fn test_authenticate_jwt_success() {
        let control_plane = Arc::new(Mutex::new(ControlPlane::new("node-1")));
        let repo = ExternalApiRepository {
            control_plane,
            jwt_secret: Some("top-secret".to_string()),
            jwt_issuer: Some("issuer-a".to_string()),
            jwt_audience: Some("wasmatrix-api".to_string()),
            mtls_principals: HashMap::new(),
        };

        let token = signed_token(
            "top-secret",
            json!({
                "sub": "client-a",
                "roles": ["instance.read", "instance.admin"],
                "iss": "issuer-a",
                "aud": "wasmatrix-api",
                "exp": Utc::now().timestamp() + 300
            }),
        );

        let principal = repo.authenticate_jwt(&token).unwrap();
        assert_eq!(principal.subject, "client-a");
        assert!(principal.roles.iter().any(|role| role == "instance.admin"));
    }

    #[test]
    fn test_authenticate_jwt_rejects_bad_signature() {
        let control_plane = Arc::new(Mutex::new(ControlPlane::new("node-1")));
        let repo = ExternalApiRepository {
            control_plane,
            jwt_secret: Some("top-secret".to_string()),
            jwt_issuer: None,
            jwt_audience: None,
            mtls_principals: HashMap::new(),
        };

        let token = signed_token(
            "different-secret",
            json!({
                "sub": "client-a",
                "roles": ["instance.read"],
                "exp": Utc::now().timestamp() + 300
            }),
        );

        let error = repo.authenticate_jwt(&token).unwrap_err();
        assert!(matches!(error, ControlPlaneError::Unauthorized(_)));
    }
}
