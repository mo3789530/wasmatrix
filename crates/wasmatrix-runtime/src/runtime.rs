use crate::capabilities::CapabilityManager;
use wasmatrix_core::{CoreError, Result};
use wasmatrix_providers::kv_provider::KvProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackend {
    Wasmtime,
    MicroKvm,
}

pub struct WasmRuntime {
    backend: RuntimeBackend,
    capabilities: CapabilityManager,
}

impl WasmRuntime {
    pub fn new() -> Self {
        Self::new_with_backend(RuntimeBackend::Wasmtime)
    }

    pub fn new_with_backend(backend: RuntimeBackend) -> Self {
        let mut capabilities = CapabilityManager::new();
        let _ = capabilities.register_provider(
            "default-kv".to_string(),
            Box::new(KvProvider::new("default-kv".to_string())),
        );
        Self {
            backend,
            capabilities,
        }
    }

    pub fn from_env() -> Self {
        let backend = match std::env::var("WASM_RUNTIME_BACKEND")
            .unwrap_or_else(|_| "wasmtime".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "micro-kvm" | "microkvm" => RuntimeBackend::MicroKvm,
            _ => RuntimeBackend::Wasmtime,
        };
        Self::new_with_backend(backend)
    }

    pub fn backend(&self) -> RuntimeBackend {
        self.backend
    }

    pub fn execute_module(&self, module_bytes: &[u8]) -> Result<String> {
        if module_bytes.len() < 4 || &module_bytes[0..4] != &[0x00, 0x61, 0x73, 0x6d] {
            return Err(CoreError::WasmRuntimeError(
                "Invalid Wasm module format".to_string(),
            ));
        }

        let message = match self.backend {
            RuntimeBackend::Wasmtime => "executed with wasmtime",
            RuntimeBackend::MicroKvm => "executed with micro-kvm",
        };
        Ok(message.to_string())
    }

    pub fn invoke_capability(
        &self,
        instance_id: &str,
        capability_id: &str,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.capabilities
            .invoke(instance_id, capability_id, operation, params)
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_wasm_module() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn test_wasm_runtime_creation_default_backend() {
        let runtime = WasmRuntime::new();
        assert_eq!(runtime.backend(), RuntimeBackend::Wasmtime);
    }

    #[test]
    fn test_runtime_can_choose_micro_kvm_backend() {
        let runtime = WasmRuntime::new_with_backend(RuntimeBackend::MicroKvm);
        assert_eq!(runtime.backend(), RuntimeBackend::MicroKvm);
    }

    #[test]
    fn test_execute_module_with_micro_kvm_backend() {
        let runtime = WasmRuntime::new_with_backend(RuntimeBackend::MicroKvm);
        let result = runtime.execute_module(&valid_wasm_module()).unwrap();
        assert_eq!(result, "executed with micro-kvm");
    }

    #[test]
    fn test_execute_module_with_wasmtime_backend() {
        let runtime = WasmRuntime::new_with_backend(RuntimeBackend::Wasmtime);
        let result = runtime.execute_module(&valid_wasm_module()).unwrap();
        assert_eq!(result, "executed with wasmtime");
    }

    #[test]
    fn test_same_capability_interface_for_both_backends() {
        let wasmtime_runtime = WasmRuntime::new_with_backend(RuntimeBackend::Wasmtime);
        let microkvm_runtime = WasmRuntime::new_with_backend(RuntimeBackend::MicroKvm);

        let params = serde_json::json!({"key":"k1","value":"v1"});
        assert!(wasmtime_runtime
            .invoke_capability("inst-1", "default-kv", "set", params.clone())
            .is_ok());
        assert!(microkvm_runtime
            .invoke_capability("inst-2", "default-kv", "set", params)
            .is_ok());
    }
}
