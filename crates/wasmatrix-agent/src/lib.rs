pub mod features;
pub mod server;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use wasmatrix_core::{
    CapabilityAssignment, CoreError, ExecutionEventRecorder, InstanceStatus, RestartPolicy,
    RestartPolicyType, Result,
};
use wasmtime::{Config, Engine, Instance, Module, Store};

/// Handle to a running Wasm instance
pub struct InstanceHandle {
    pub instance_id: String,
    pub store: Store<()>,
    pub instance: Instance,
    pub module_bytes: Vec<u8>,
    pub capabilities: Vec<CapabilityAssignment>,
    pub restart_policy: RestartPolicy,
}

/// Crash information for restart policy evaluation
#[derive(Debug, Clone)]
pub struct CrashInfo {
    pub crash_count: u32,
    pub last_crash_time: Option<std::time::Instant>,
}

impl CrashInfo {
    pub fn new() -> Self {
        Self {
            crash_count: 0,
            last_crash_time: None,
        }
    }

    pub fn record_crash(&mut self) {
        self.crash_count += 1;
        self.last_crash_time = Some(std::time::Instant::now());
    }

    /// Calculate backoff delay based on crash count
    pub fn calculate_backoff(&self, base_seconds: u64) -> u64 {
        // Exponential backoff: base * 2^(crash_count - 1), capped at 5 minutes
        let exponent = self.crash_count.saturating_sub(1);
        let delay = base_seconds * 2_u64.pow(exponent.min(8)); // Cap at 256x base
        delay.min(300) // Cap at 5 minutes
    }
}

impl Default for CrashInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Restart policy evaluator
pub struct RestartPolicyEvaluator;

impl RestartPolicyEvaluator {
    /// Evaluate whether an instance should be restarted based on policy and crash history
    pub fn should_restart(
        policy: &RestartPolicy,
        crash_info: &CrashInfo,
    ) -> Option<std::time::Duration> {
        match policy.policy_type {
            RestartPolicyType::Never => {
                info!("Restart policy is 'never', not restarting instance");
                None
            }
            RestartPolicyType::Always => {
                info!("Restart policy is 'always', restarting instance immediately");
                Some(std::time::Duration::from_secs(0))
            }
            RestartPolicyType::OnFailure => {
                // Check if we've exceeded max retries (crash count includes current crash)
                if let Some(max_retries) = policy.max_retries {
                    if crash_info.crash_count > max_retries {
                        warn!(
                            crash_count = crash_info.crash_count,
                            max_retries = max_retries,
                            "Maximum retry count exceeded, not restarting"
                        );
                        return None;
                    }
                }

                // Calculate backoff delay
                let backoff_seconds = policy.backoff_seconds.unwrap_or(5);
                let delay = crash_info.calculate_backoff(backoff_seconds);
                info!(delay_seconds = delay, "Restarting instance with backoff");
                Some(std::time::Duration::from_secs(delay))
            }
        }
    }
}

/// Node Agent manages local Wasm instance execution
pub struct NodeAgent {
    engine: Engine,
    instances: Arc<RwLock<HashMap<String, InstanceHandle>>>,
    crash_history: Arc<RwLock<HashMap<String, CrashInfo>>>,
    event_recorder: Arc<RwLock<ExecutionEventRecorder>>,
    crashed_instances: Arc<RwLock<HashMap<String, std::time::Instant>>>,
    node_id: String,
}

impl NodeAgent {
    pub fn new(node_id: impl Into<String>) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);

        let engine = Engine::new(&config).map_err(|e| {
            CoreError::InvalidInstanceId(format!("Failed to create wasmtime engine: {}", e))
        })?;

        Ok(Self {
            engine,
            instances: Arc::new(RwLock::new(HashMap::new())),
            crash_history: Arc::new(RwLock::new(HashMap::new())),
            event_recorder: Arc::new(RwLock::new(ExecutionEventRecorder::new())),
            crashed_instances: Arc::new(RwLock::new(HashMap::new())),
            node_id: node_id.into(),
        })
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Start a Wasm instance locally
    pub async fn start_instance_local(
        &self,
        instance_id: String,
        module_bytes: Vec<u8>,
        capabilities: Vec<CapabilityAssignment>,
        restart_policy: RestartPolicy,
    ) -> Result<()> {
        // Validate module bytes
        if module_bytes.len() < 4 || &module_bytes[0..4] != &[0x00, 0x61, 0x73, 0x6d] {
            return Err(CoreError::InvalidInstanceId(
                "Invalid Wasm module format".to_string(),
            ));
        }

        // Compile module
        let module = Module::new(&self.engine, &module_bytes).map_err(|e| {
            CoreError::InvalidInstanceId(format!("Failed to compile Wasm module: {}", e))
        })?;

        // Create store with WASI context
        let mut store = Store::new(&self.engine, ());

        // Instantiate the module
        let instance = Instance::new(&mut store, &module, &[]).map_err(|e| {
            CoreError::InvalidInstanceId(format!("Failed to instantiate Wasm module: {}", e))
        })?;

        info!(instance_id = %instance_id, "Wasm instance started successfully");

        // Record start event
        {
            let mut recorder = self.event_recorder.write().await;
            recorder.record_start(&instance_id);
        }

        // Store the handle
        let handle = InstanceHandle {
            instance_id: instance_id.clone(),
            store,
            instance,
            module_bytes,
            capabilities,
            restart_policy,
        };

        let mut instances = self.instances.write().await;
        instances.insert(instance_id, handle);

        Ok(())
    }

    /// Stop a running Wasm instance
    pub async fn stop_instance_local(&self, instance_id: &str) -> Result<()> {
        let mut instances = self.instances.write().await;

        if instances.remove(instance_id).is_some() {
            info!(instance_id = %instance_id, "Wasm instance stopped");

            // Remove from crashed instances (if present)
            {
                let mut crashed = self.crashed_instances.write().await;
                crashed.remove(instance_id);
            }

            // Record stop event
            {
                let mut recorder = self.event_recorder.write().await;
                recorder.record_stop(instance_id);
            }

            Ok(())
        } else {
            Err(CoreError::InvalidInstanceId(format!(
                "Instance {} not found",
                instance_id
            )))
        }
    }

    /// Handle instance crash detection
    pub async fn on_instance_crash(
        &self,
        instance_id: &str,
        error: String,
    ) -> Option<std::time::Duration> {
        error!(instance_id = %instance_id, error = %error, "Instance crashed");

        // Record crash event in execution event recorder
        {
            let mut recorder = self.event_recorder.write().await;
            recorder.record_crash(instance_id, &error);
        }

        // Mark instance as crashed
        {
            let mut crashed = self.crashed_instances.write().await;
            crashed.insert(instance_id.to_string(), std::time::Instant::now());
        }

        // Record crash in history
        let mut crash_history = self.crash_history.write().await;
        let crash_info = crash_history
            .entry(instance_id.to_string())
            .or_insert_with(CrashInfo::new);
        crash_info.record_crash();

        // Get the instance's restart policy
        let instances = self.instances.read().await;
        if let Some(handle) = instances.get(instance_id) {
            let policy = &handle.restart_policy;
            let delay = RestartPolicyEvaluator::should_restart(policy, crash_info);

            if delay.is_some() {
                info!(instance_id = %instance_id, "Instance will be restarted according to policy");
            } else {
                info!(instance_id = %instance_id, "Instance will not be restarted according to policy");
            }

            delay
        } else {
            warn!(instance_id = %instance_id, "Crashed instance not found in active instances");
            None
        }
    }

    /// Get instance status
    pub async fn get_instance_status(&self, instance_id: &str) -> InstanceStatus {
        // Check if crashed first (highest priority status)
        {
            let crashed = self.crashed_instances.read().await;
            if crashed.contains_key(instance_id) {
                return InstanceStatus::Crashed;
            }
        }

        // Check if running
        let instances = self.instances.read().await;
        if instances.contains_key(instance_id) {
            InstanceStatus::Running
        } else {
            InstanceStatus::Stopped
        }
    }

    /// Get crash count for an instance
    pub async fn get_crash_count(&self, instance_id: &str) -> u32 {
        let crash_history = self.crash_history.read().await;
        crash_history
            .get(instance_id)
            .map(|info| info.crash_count)
            .unwrap_or(0)
    }

    /// Restart an instance (internal use)
    pub async fn restart_instance(&self, instance_id: &str) -> Result<()> {
        let instances = self.instances.read().await;

        if let Some(handle) = instances.get(instance_id) {
            let module_bytes = handle.module_bytes.clone();
            let capabilities = handle.capabilities.clone();
            let restart_policy = handle.restart_policy.clone();
            drop(instances);

            // Remove from crashed instances (if present)
            {
                let mut crashed = self.crashed_instances.write().await;
                crashed.remove(instance_id);
            }

            // Stop the old instance
            self.stop_instance_local(instance_id).await?;

            // Start a new instance with the same parameters
            self.start_instance_local(
                instance_id.to_string(),
                module_bytes,
                capabilities,
                restart_policy,
            )
            .await?;

            // Record restart event
            {
                let mut recorder = self.event_recorder.write().await;
                recorder.record_restart(instance_id);
            }

            info!(instance_id = %instance_id, "Instance restarted successfully");
            Ok(())
        } else {
            Err(CoreError::InvalidInstanceId(format!(
                "Instance {} not found for restart",
                instance_id
            )))
        }
    }

    /// List all running instances
    pub async fn list_instances(&self) -> Vec<String> {
        let instances = self.instances.read().await;
        instances.keys().cloned().collect()
    }

    /// Get execution events for monitoring and debugging
    pub async fn get_execution_events(&self) -> Vec<wasmatrix_core::ExecutionEvent> {
        let recorder = self.event_recorder.read().await;
        recorder.get_events().to_vec()
    }

    /// Get execution events for a specific instance
    pub async fn get_execution_events_for_instance(
        &self,
        instance_id: &str,
    ) -> Vec<wasmatrix_core::ExecutionEvent> {
        let recorder = self.event_recorder.read().await;
        recorder
            .get_events_for_instance(instance_id)
            .into_iter()
            .map(|e| e.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_wasm_module() -> Vec<u8> {
        // Minimal valid Wasm module (magic bytes + version)
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[tokio::test]
    async fn test_start_stop_instance() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::default(),
            )
            .await
            .unwrap();

        // Verify it's running
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Running);

        // Stop instance
        agent.stop_instance_local(&instance_id).await.unwrap();

        // Verify it's stopped
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Stopped);
    }

    #[tokio::test]
    async fn test_invalid_wasm_module() {
        let agent = NodeAgent::new("test-node").unwrap();
        let result = agent
            .start_instance_local(
                "test".to_string(),
                vec![0x00, 0x00, 0x00, 0x00],
                vec![],
                RestartPolicy::default(),
            )
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_restart_policy_never() {
        let policy = RestartPolicy::never();
        let crash_info = CrashInfo::new();

        let result = RestartPolicyEvaluator::should_restart(&policy, &crash_info);
        assert!(result.is_none());
    }

    #[test]
    fn test_restart_policy_always() {
        let policy = RestartPolicy::always();
        let crash_info = CrashInfo::new();

        let result = RestartPolicyEvaluator::should_restart(&policy, &crash_info);
        assert_eq!(result, Some(std::time::Duration::from_secs(0)));
    }

    #[test]
    fn test_restart_policy_on_failure() {
        let policy = RestartPolicy::on_failure(3, 5);
        let mut crash_info = CrashInfo::new();

        // First crash - should restart with 5s delay
        crash_info.record_crash();
        let result = RestartPolicyEvaluator::should_restart(&policy, &crash_info);
        assert_eq!(result, Some(std::time::Duration::from_secs(5)));

        // Second crash - should restart with 10s delay (exponential backoff)
        crash_info.record_crash();
        let result = RestartPolicyEvaluator::should_restart(&policy, &crash_info);
        assert_eq!(result, Some(std::time::Duration::from_secs(10)));

        // Third crash - should restart with 20s delay
        crash_info.record_crash();
        let result = RestartPolicyEvaluator::should_restart(&policy, &crash_info);
        assert_eq!(result, Some(std::time::Duration::from_secs(20)));

        // Fifth crash - exceeds max_retries (3), should not restart
        crash_info.record_crash();
        let result = RestartPolicyEvaluator::should_restart(&policy, &crash_info);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_crash_detection() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance with "always" restart policy
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::always(),
            )
            .await
            .unwrap();

        // Simulate crash
        let delay = agent
            .on_instance_crash(&instance_id, "test error".to_string())
            .await;

        // Should restart immediately (always policy)
        assert_eq!(delay, Some(std::time::Duration::from_secs(0)));

        // Verify crash was recorded
        let crash_count = agent.get_crash_count(&instance_id).await;
        assert_eq!(crash_count, 1);
    }

    #[tokio::test]
    async fn test_list_instances() {
        let agent = NodeAgent::new("test-node").unwrap();

        // Start multiple instances
        for i in 0..3 {
            agent
                .start_instance_local(
                    format!("instance-{}", i),
                    create_valid_wasm_module(),
                    vec![],
                    RestartPolicy::default(),
                )
                .await
                .unwrap();
        }

        let instances = agent.list_instances().await;
        assert_eq!(instances.len(), 3);
    }

    #[test]
    fn test_crash_info_backoff_calculation() {
        let mut crash_info = CrashInfo::new();

        // Test exponential backoff
        crash_info.record_crash();
        assert_eq!(crash_info.calculate_backoff(5), 5); // 5 * 2^0

        crash_info.record_crash();
        assert_eq!(crash_info.calculate_backoff(5), 10); // 5 * 2^1

        crash_info.record_crash();
        assert_eq!(crash_info.calculate_backoff(5), 20); // 5 * 2^2

        crash_info.record_crash();
        assert_eq!(crash_info.calculate_backoff(5), 40); // 5 * 2^3

        // Test capping at 300 seconds
        for _ in 0..10 {
            crash_info.record_crash();
        }
        assert_eq!(crash_info.calculate_backoff(5), 300); // capped at 300
    }

    #[tokio::test]
    async fn test_crash_recovery_event_recording() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance with "always" restart policy
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::always(),
            )
            .await
            .unwrap();

        // Simulate crash and record event
        agent
            .on_instance_crash(&instance_id, "panic in module".to_string())
            .await;

        // Check that both start and crash events were recorded
        let events = agent.get_execution_events_for_instance(&instance_id).await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "instance_started");
        assert_eq!(events[1].event_type, "instance_crashed");
        assert!(events[1].details.is_some());

        // Restart instance
        agent.restart_instance(&instance_id).await.unwrap();

        // Check that all events were recorded: start, crash, stop, start, restart
        // The stop event comes from restart_instance calling stop_instance_local
        let events = agent.get_execution_events_for_instance(&instance_id).await;
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].event_type, "instance_started");
        assert_eq!(events[1].event_type, "instance_crashed");
        assert_eq!(events[2].event_type, "instance_stopped");
        assert_eq!(events[3].event_type, "instance_started");
        assert_eq!(events[4].event_type, "instance_restarted");
    }

    #[tokio::test]
    async fn test_state_preservation_during_crash() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance with "on_failure" policy
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::on_failure(3, 5),
            )
            .await
            .unwrap();

        // Record initial state
        let initial_instances = agent.list_instances().await;
        assert_eq!(initial_instances.len(), 1);
        assert!(initial_instances.contains(&instance_id));

        // Simulate crash
        agent
            .on_instance_crash(&instance_id, "test error".to_string())
            .await;

        // Verify system-level state is preserved
        // The crash history should be preserved
        let crash_count = agent.get_crash_count(&instance_id).await;
        assert_eq!(crash_count, 1);

        // The execution events should be preserved (start + crash)
        let events = agent.get_execution_events().await;
        assert_eq!(events.len(), 2);

        // Restart and verify state continuity
        agent.restart_instance(&instance_id).await.unwrap();
        let crash_count_after_restart = agent.get_crash_count(&instance_id).await;
        // Crash count should still be 1 (we only had one crash before restart)
        assert_eq!(crash_count_after_restart, 1);
    }

    #[tokio::test]
    async fn test_multiple_instance_crashes() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::on_failure(3, 5),
            )
            .await
            .unwrap();

        // Simulate multiple crashes
        for i in 1..=3 {
            agent
                .on_instance_crash(&instance_id, format!("crash {}", i))
                .await;
        }

        // Check crash count
        let crash_count = agent.get_crash_count(&instance_id).await;
        assert_eq!(crash_count, 3);

        // Check that all crashes were recorded (start + 3 crashes = 4 events)
        let events = agent.get_execution_events_for_instance(&instance_id).await;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].event_type, "instance_started");
        for event in &events[1..] {
            assert_eq!(event.event_type, "instance_crashed");
        }

        // Verify restart policy is still enforced after multiple crashes
        let delay = agent
            .on_instance_crash(&instance_id, "crash 4".to_string())
            .await;
        // With max_retries=3 and 4 crashes, should not restart
        assert!(delay.is_none());
    }

    #[tokio::test]
    async fn test_get_all_execution_events() {
        let agent = NodeAgent::new("test-node").unwrap();

        // Start multiple instances
        for i in 0..3 {
            agent
                .start_instance_local(
                    format!("instance-{}", i),
                    create_valid_wasm_module(),
                    vec![],
                    RestartPolicy::always(),
                )
                .await
                .unwrap();
        }

        // Simulate crashes on all instances
        for i in 0..3 {
            agent
                .on_instance_crash(&format!("instance-{}", i), format!("crash {}", i))
                .await;
        }

        // Get all events
        let all_events = agent.get_execution_events().await;
        assert_eq!(all_events.len(), 6);

        // Verify first 3 are start events, last 3 are crash events
        for event in &all_events[..3] {
            assert_eq!(event.event_type, "instance_started");
        }
        for event in &all_events[3..] {
            assert_eq!(event.event_type, "instance_crashed");
        }
    }

    #[tokio::test]
    async fn test_actual_status_not_intended_status() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance - this records "instance_started" event
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::default(),
            )
            .await
            .unwrap();

        // Verify status query returns actual Running status
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Running);

        // Verify we have a start event
        let events = agent.get_execution_events_for_instance(&instance_id).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "instance_started");

        // Stop instance - this records "instance_stopped" event
        agent.stop_instance_local(&instance_id).await.unwrap();

        // Verify status query now returns actual Stopped status (not any intended state)
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Stopped);

        // Verify we have both start and stop events
        let events = agent.get_execution_events_for_instance(&instance_id).await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "instance_started");
        assert_eq!(events[1].event_type, "instance_stopped");

        // Status query should continue to return Stopped (actual state)
        // There's no reconciliation logic to change it to Running or any intended state
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Stopped);
    }

    #[tokio::test]
    async fn test_no_reconciliation_logic() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Start instance
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::never(),
            )
            .await
            .unwrap();

        // Get initial status (Running)
        let status1 = agent.get_instance_status(&instance_id).await;
        assert_eq!(status1, InstanceStatus::Running);

        // Simulate crash
        agent
            .on_instance_crash(&instance_id, "test error".to_string())
            .await;

        // Status should now be Crashed (actual state after crash)
        // There's no reconciliation logic to restart it or change status
        let status2 = agent.get_instance_status(&instance_id).await;
        assert_eq!(status2, InstanceStatus::Crashed);

        // Even with "always" restart policy, we need explicit restart
        // The system doesn't automatically reconcile to desired Running state
        // (This is by design - execution facts model)
    }

    #[tokio::test]
    async fn test_status_queries_based_on_actual_state() {
        let agent = NodeAgent::new("test-node").unwrap();
        let instance_id = "test-instance-1".to_string();

        // Initially, instance doesn't exist
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Stopped);

        // Start instance
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::always(),
            )
            .await
            .unwrap();

        // Now it's Running (actual state)
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Running);

        // Stop instance
        agent.stop_instance_local(&instance_id).await.unwrap();

        // Now it's Stopped (actual state)
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Stopped);

        // Start again
        agent
            .start_instance_local(
                instance_id.clone(),
                create_valid_wasm_module(),
                vec![],
                RestartPolicy::always(),
            )
            .await
            .unwrap();

        // Now it's Running again (actual state)
        let status = agent.get_instance_status(&instance_id).await;
        assert_eq!(status, InstanceStatus::Running);
    }
}
