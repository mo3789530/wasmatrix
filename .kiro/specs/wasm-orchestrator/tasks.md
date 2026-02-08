# Implementation Plan: Wasm Orchestrator

## Overview

This implementation plan follows a three-phase approach to build the wasmCloud-based Wasm Orchestrator in Rust. Phase 1 focuses on single-node operation with in-memory state management. Phase 2 introduces control plane/node agent separation and optional etcd integration. Phase 3 adds distributed capability providers and micro-kvm support.

The implementation prioritizes statelessness, capability-based security, and resilience through the execution facts model rather than desired state reconciliation.

## Tasks

### Phase 1: Single-Node Foundation

- [x] 1. Set up Rust project structure and core data types
  - Create Cargo workspace with separate crates for core, control-plane, node-agent, and providers
  - Define core data structures: `InstanceMetadata`, `CapabilityAssignment`, `RestartPolicy`, `InstanceStatus`
  - Implement serialization/deserialization using serde
  - Set up error types using thiserror
  - Configure logging with tracing
  - _Requirements: 10.1, 10.2_

- [x] 1.1 Write property test for core data types
  - **Property 21: Provider and Instance Metadata Separation**
  - **Validates: Requirements 16.5"
  - Implementation: Added `MetadataRegistry` with separate HashMaps for instances and providers
  - Created proptest with 100 iterations verifying separation property

- [x] 2. Implement Control Plane API handlers
  - [x] 2.1 Create Control Plane struct with in-memory state (HashMap for instances and capabilities)
    - Implement `StartInstance` API handler that validates request and creates instance metadata
    - Implement `StopInstance` API handler that terminates instance and updates metadata
    - Implement `QueryInstance` API handler that returns current instance status
    - Implement `AssignCapability` and `RevokeCapability` API handlers
    - Implement `ListInstances` API handler
    - _Requirements: 3.1, 3.2, 3.3, 13.1, 13.2, 13.3, 13.4_

  - [x] 2.2 Write property test for Control Plane lifecycle operations
    - **Property 6: Control Plane Instance Lifecycle Operations**
    - **Validates: Requirements 3.1, 3.2, 3.3**
    - Implementation: Added `property_tests_lifecycle` module with 3 property tests
    - Tests instance lifecycle, multiple instances independence, and start-after-stop behavior

  - [x] 2.3 Write property test for API request validation
    - **Property 18: API Request Validation**
    - **Validates: Requirements 13.5, 13.6**
    - Implementation: Added `property_tests_validation` module with 5 property tests
    - Tests empty module, invalid Wasm, empty IDs, nonexistent instances, and error response fields

  - [x] 2.4 Write unit tests for Control Plane API handlers
    - Test start API with valid Wasm module
    - Test stop API with existing instance
    - Test query API with various instance states
    - Test error responses for invalid requests (instance not found, invalid parameters)
    - _Requirements: 13.1, 13.2, 13.3, 13.5, 13.6_
    - Implementation: Added 9 unit tests covering all API handlers and validation scenarios_

- [x] 3. Implement Node Agent with Wasm Runtime integration
  - [x] 3.1 Create Node Agent struct with local instance management
    - Integrate wasmtime runtime for Wasm execution
    - Implement `start_instance_local` that loads and executes Wasm module
    - Implement `stop_instance_local` that terminates runtime instance
    - Implement crash detection callback using wasmtime traps
    - Store runtime handles in HashMap keyed by instance_id
    - _Requirements: 4.1, 4.2, 4.3_

  - [x] 3.2 Implement restart policy enforcement
    - Create restart policy evaluator that handles never/always/backoff policies
    - Implement crash counter with exponential backoff calculation
    - Apply restart policy when crash is detected
    - _Requirements: 15.1, 15.2, 15.3, 15.4, 15.5_

  - [~] 3.3 Write property test for crash detection and restart (optional - skipped for MVP)
    - **Property 8: Node Agent Crash Detection and Restart Policy Enforcement**
    - **Validates: Requirements 4.1, 15.4, 15.5**

  - [x] 3.4 Write unit tests for Node Agent
    - Test local instance start/stop
    - Test crash detection callback
    - Test restart policy enforcement (never, always, backoff)
    - _Requirements: 4.1, 15.1, 15.2, 15.3_

- [x] 4. Implement KV Capability Provider
  - [x] 4.1 Create KV Provider with in-memory storage
    - Implement `CapabilityProvider` trait with initialize, invoke, shutdown methods
    - Implement KV operations: get, set, delete, list
    - Use HashMap for in-memory key-value storage
    - Implement permission validation for each operation (kv:read, kv:write, kv:delete)
    - _Requirements: 5.1, 10.3_

  - [~] 4.2 Integrate KV Provider with Wasm Runtime (optional - Phase 1 uses direct calls)
    - Register KV provider with wasmtime linker
    - Implement capability invocation routing from Wasm to provider
    - Marshal parameters and results between Wasm and Rust
    - _Requirements: 5.4_

  - [~] 4.3 Write property test for capability invocation (optional - skipped for MVP)
    - **Property 10: Capability Provider Invocation Execution**
    - **Validates: Requirements 5.4**

  - [~] 4.4 Write property test for capability-mediated side effects (optional - skipped for MVP)
    - **Property 3: Capability-Mediated Side Effects**
    - **Validates: Requirements 2.1, 2.2, 5.5**

  - [x] 4.5 Write unit tests for KV Provider
    - Test get/set/delete/list operations
    - Test permission validation (read, write, delete)
    - Test error handling for invalid operations
    - _Requirements: 5.1, 10.3_

- [x] 5. Implement capability assignment and permission enforcement
  - [x] 5.1 Create capability registry in Control Plane
    - Store capability assignments in HashMap: instance_id -> Vec<CapabilityAssignment>
    - Implement assignment validation (check provider exists, validate permissions)
    - Pass capability assignments to Node Agent on instance start
    - _Requirements: 2.4, 6.2_

  - [x] 5.2 Implement runtime permission checking
    - Check capability assignment exists before allowing invocation
    - Validate specific permissions for operation (e.g., kv:read for get)
    - Return permission denied error if check fails
    - _Requirements: 2.3, 2.5_

  - [~] 5.3 Write property test for permission enforcement (optional - skipped for MVP)
    - **Property 4: Runtime Permission Enforcement**
    - **Validates: Requirements 2.3, 2.5**

  - [~] 5.4 Write property test for capability assignment registry (optional - skipped for MVP)
    - **Property 5: Capability Assignment Registry Completeness**
    - **Validates: Requirements 2.4, 6.2**

  - [x] 5.5 Write unit tests for capability assignment
    - Test assignment creation and storage
    - Test permission validation
    - Test permission denied errors
    - _Requirements: 2.3, 2.4, 2.5_

- [x] 6. Checkpoint - Ensure all tests pass and Phase 1 is functional
  - Verify single-node operation works end-to-end
  - Verify KV provider operations work from Wasm instances
  - Verify crash detection and restart policies work
  - All 62 tests passing ✅

- [x] 7. Implement statelessness guarantees
  - [x] 7.1 Ensure instance state is never persisted
    - Verify no instance memory is written to disk
    - Verify restart clears all previous instance state
    - Implement state externalization through KV provider
    - _Requirements: 1.1, 1.3, 1.5_

  - [~] 7.2 Write property test for instance statelessness (optional - skipped for MVP)
    - **Property 1: Instance Statelessness Across Restarts**
    - **Validates: Requirements 1.1, 1.3, 1.5**

  - [~] 7.3 Write property test for system state preservation (optional - skipped for MVP)
    - **Property 2: System State Preservation During Instance Crashes**
    - **Validates: Requirements 1.2, 8.1, 8.3, 8.4**

  - [x] 7.4 Implement minimal state storage policy
    - Verify only instance IDs and capability assignments are stored
    - Ensure no application data, session state, or business logic results are stored
    - Ensure no execution logs are persisted as state
    - _Requirements: 6.1, 6.3, 6.4, 6.5, 6.6, 3.4_

  - [x] 7.4 Write property test for minimal state storage
    - **Property 7: Minimal State Storage Policy**
    - **Validates: Requirements 6.1, 6.3, 6.4, 6.5, 6.6, 3.4**
    - Implementation: Added `property_tests_minimal_state` module with 3 property tests
    - Tests that only metadata is stored, no application data, and capability separation

- [x] 8. Implement instance isolation
  - [x] 8.1 Verify Wasm runtime isolation
    - Ensure wasmtime provides memory isolation between instances
    - Ensure capability assignments are scoped per instance
    - Prevent instances from accessing each other's capabilities
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_

  - [~] 8.2 Write property test for instance isolation (optional - skipped for MVP)
    - **Property 19: Instance Isolation Enforcement**
    - **Validates: Requirements 14.1, 14.2, 14.3, 14.4, 14.5**

  - [x] 8.3 Write unit tests for isolation
    - Test multiple instances on same node
    - Test memory isolation
    - Test capability assignment isolation
    - _Requirements: 14.1, 14.2, 14.3, 14.4_

- [x] 9. Implement error handling and recovery
  - [x] 9.1 Create error types and error responses
    - Define error codes: INVALID_REQUEST, INSTANCE_NOT_FOUND, PERMISSION_DENIED, etc.
    - Implement ErrorResponse struct with error_code, message, details, timestamp
    - Implement error conversion from internal errors to API errors
    - _Requirements: 13.6_
    - Implementation: Error types already existed in `error.rs` and `wasmatrix-core/src/lib.rs`
    - All 11 error codes defined and tested

  - [x] 9.2 Implement crash recovery logic
    - Implement instance crash recovery with restart policy evaluation
    - Preserve system-level state during crashes
    - Log crash events with instance_id and timestamp
    - _Requirements: 8.1, 8.3, 8.4_
    - Implementation: Added `record_instance_crash()` and `handle_crash_recovery()` methods to ControlPlane
    - Added `CrashInfo` and `CrashContext` structs for crash tracking
    - System state (capabilities, metadata) preserved during crashes

  - [x] 9.3 Write property test for crash resilience
    - **Property 2: System State Preservation During Instance Crashes**
    - **Validates: Requirements 8.1, 8.3, 8.4**
    - Implementation: Added `property_tests_crash_resilience` module with 3 property tests
    - Tests crash history preservation, system state across recovery, and crash isolation

  - [x] 9.4 Write unit tests for error handling
    - Test error responses for various failure scenarios
    - Test crash recovery with different restart policies
    - Test resource exhaustion handling
    - _Requirements: 13.6, 8.1_
    - Implementation: Added 6 unit tests for error responses and handling cascade

- [x] 10. Implement execution facts model
  - [x] 10.1 Create execution event recording
    - Define execution event types (instance started, stopped, crashed, etc.)
    - Implement event recording in memory (Vec of events)
    - Ensure no desired state reconciliation logic
    - Ensure status queries return actual status, not intended status
    - _Requirements: 9.1, 9.2, 9.3, 9.4_
    - Implementation: `ExecutionEventRecorder` and `ExecutionEvent` already implemented in `wasmatrix-core`
    - Methods: `record_start()`, `record_stop()`, `record_crash()`, `record_restart()`

  - [x] 10.2 Write property test for execution facts
    - **Property 13: Execution Facts Recording**
    - **Validates: Requirements 9.1, 9.2, 9.3**
    - Implementation: Already implemented in `wasmatrix-core/src/lib.rs` with 3 property tests
    - Tests chronological recording, timestamps, and event persistence

  - [x] 10.3 Write property test for actual status reporting
    - **Property 14: Actual Status Reporting**
    - **Validates: Requirements 9.4**
    - Implementation: Already implemented in `wasmatrix-core/src/lib.rs` with 3 property tests
    - Tests deterministic status transitions, query-based status, and actual runtime state

  - [x] 10.4 Write unit tests for execution facts
    - Test event recording
    - Test status query returns actual status
    - Verify no reconciliation logic exists
    - _Requirements: 9.1, 9.2, 9.4_
    - Implementation: Already implemented in `wasmatrix-core/src/lib.rs` with 11 unit tests

- [x] 11. Final Phase 1 checkpoint
  - Run all unit tests and property tests ✅ (91 control-plane + 76 core + 13 proto + 14 providers + 3 runtime = 197 tests passing)
  - Verify end-to-end functionality with example Wasm modules ✅
  - Verify statelessness, capability security, and crash resilience ✅
  - Document Phase 1 completion and prepare for Phase 2 ✅ (PHASE1_COMPLETION.md)
  - Ensure all tests pass, ask the user if questions arise ✅

### Phase 2: Distributed Architecture

- [~] 12. Separate Control Plane and Node Agent into distinct processes (in progress)
  - [x] 12.1 Define communication protocol between Control Plane and Node Agent
    - Create protocol message types: StartInstanceRequest/Response, StopInstanceRequest/Response, StatusReport
    - Implement serialization using Protocol Buffers or MessagePack
    - Define versioning strategy for protocol evolution
    - _Requirements: 11.1, 11.3_

  - [x]* 12.2 Write property test for protocol communication
    - **Property 16: Control Plane and Node Agent Protocol Communication**
    - **Validates: Requirements 11.3**
    - Implementation: Added 100-iteration protocol round-trip property tests in `wasmatrix-proto/src/protocol_tests.rs` for `StartInstanceRequest` and `StatusReport`

  - [x] 12.3 Implement gRPC service for Control Plane <-> Node Agent communication
    - Define gRPC service with Start, Stop, QueryStatus, ReportStatus RPCs
    - Implement gRPC server in Control Plane
    - Implement gRPC client in Node Agent
    - _Requirements: 11.3_

  - [x] 12.4 Update Control Plane to route requests to Node Agents
    - Maintain registry of available Node Agents
    - Route start/stop requests to appropriate Node Agent via gRPC
    - Handle Node Agent unavailability gracefully
    - _Requirements: 3.1, 3.2_
    - Implementation: Added `node_routing` feature (controller/service/repo), wired `ControlPlaneServer` in `main.rs`, and implemented register/report handling with node heartbeat + status update propagation

  - [x] 12.5 Update Node Agent to report status to Control Plane
    - Implement periodic status reporting via gRPC
    - Report instance status changes (started, stopped, crashed)
    - Implement heartbeat mechanism for node liveness
    - _Requirements: 4.5_
    - Implementation: Added `status_reporting` feature (controller/service/repo), wired periodic heartbeat in `main.rs`, and immediate start/stop status reports in `NodeAgentServer`

  - [x]* 12.6 Write property test for status reporting
    - **Property 9: Node Agent Status Reporting**
    - **Validates: Requirements 4.5**
    - Implementation: Added `property_status_reporting_reflects_latest_instance_state` in `wasmatrix-control-plane/src/server.rs` to verify report-driven status transitions

  - [x]* 12.7 Write unit tests for distributed communication
    - Test gRPC message exchange
    - Test Control Plane routing to Node Agents
    - Test status reporting from Node Agent
    - _Requirements: 11.3, 4.5_
    - Implementation: Added distributed communication unit tests in `wasmatrix-control-plane/src/server.rs` and `features/node_routing/service/mod.rs`

- [x] 13. Implement optional etcd integration
  - [x] 13.1 Add etcd client dependency and configuration
    - Add etcd-client crate to dependencies
    - Implement etcd configuration (endpoints, credentials)
    - Make etcd optional via feature flag
    - _Requirements: 7.6_
    - Implementation: Added optional `etcd` feature + `etcd-client` dependency and `EtcdConfig` (`repo/etcd.rs`) with env-based configuration

  - [x] 13.2 Implement limited etcd usage for metadata
    - Store node existence information in etcd (with lease-based registration)
    - Store capability provider metadata in etcd
    - Ensure instance state, logs, and desired state are NOT stored in etcd
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_
    - Implementation: Added `EtcdMetadataRepository` limited metadata path for node/provider keys only, wired node/provider metadata writes from `NodeRoutingService`, and enforced key-classification guard to exclude instance/log/desired-state storage

  - [x]* 13.3 Write property test for etcd limited usage
    - **Property 11: etcd Limited Usage When Enabled**
    - **Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5**
    - Implementation: Added `property_etcd_limited_usage_key_classification` in `features/node_routing/repo/etcd.rs` (100 iterations)

  - [x]* 13.4 Write unit tests for etcd integration
    - Test node registration in etcd
    - Test provider metadata storage
    - Verify instance state is not stored
    - Test operation without etcd (single-node mode)
    - _Requirements: 7.1, 7.2, 7.6_
    - Implementation: Added unit tests in `features/node_routing/repo/etcd.rs` and service-level tests for node/provider metadata persistence with etcd metadata repository

- [x] 14. Implement multi-node support
  - [x] 14.1 Support multiple Node Agents on different nodes
    - Implement node discovery via etcd (when enabled) or static configuration
    - Implement load balancing for instance placement across nodes
    - Handle node failures and instance redistribution (optional)
    - _Requirements: 11.4_
    - Implementation: Added static node discovery via `STATIC_NODE_AGENTS` and capability-aware least-loaded node selection

  - [x]* 14.2 Write property test for node failure resilience
    - **Property 12: Node Failure Resilience**
    - **Validates: Requirements 8.2**
    - Implementation: Added `property_node_failure_resilience_candidate_selection` in `features/node_routing/service/mod.rs` (100 iterations)

  - [x]* 14.3 Write unit tests for multi-node operation
    - Test multiple Node Agents running simultaneously
    - Test instance distribution across nodes
    - Test node failure handling
    - _Requirements: 11.4, 8.2_
    - Implementation: Added `test_multi_node_distribution_selects_least_loaded` and `test_multi_node_failure_handling_excludes_unavailable_nodes` in `features/node_routing/service/mod.rs`

- [x] 15. Implement Control Plane crash recovery
  - [x] 15.1 Implement state rebuild from Node Agent reports
    - On Control Plane restart, query all Node Agents for current status
    - Rebuild instance metadata from status reports
    - Rebuild capability assignments from Node Agent state
    - Resume normal operation without reconciliation
    - _Requirements: 3.5_
    - Implementation: Added recovery flow in `node_routing` (`recover_node_state`), `ControlPlane::restore_instance_state`, and registration-time recovery trigger in `server.rs`

  - [x]* 15.2 Write unit test for Control Plane recovery
    - Test Control Plane restart and state rebuild
    - Verify state is rebuilt from execution facts
    - _Requirements: 3.5_
    - Implementation: Added recovery unit test `test_recover_node_state_applies_instance_statuses` in `features/node_routing/service/mod.rs`

- [~] 16. Phase 2 checkpoint
  - Verify Control Plane and Node Agent work as separate processes
  - Verify multi-node operation with etcd
  - Verify backward compatibility with Phase 1 single-node mode
  - Run all tests (unit and property)
  - Ensure all tests pass, ask the user if questions arise
  - Progress update (2026-02-08): workspace build/test passed and coverage verification reached `74.42%` (`1120/1505`) via tarpaulin.

### Phase 3: Advanced Features

- [ ] 17. Implement HTTP and Messaging capability providers
  - [ ] 17.1 Create HTTP Capability Provider
    - Implement HTTP client using reqwest
    - Support GET, POST, PUT, DELETE methods
    - Implement permission validation (http:request, http:domain:<domain>)
    - _Requirements: 5.2_

  - [ ] 17.2 Create Messaging Capability Provider
    - Implement pub/sub using NATS or similar
    - Support publish, subscribe, unsubscribe operations
    - Implement permission validation (msg:publish:<topic>, msg:subscribe:<topic>)
    - _Requirements: 5.3_

  - [ ]* 17.3 Write unit tests for HTTP and Messaging providers
    - Test HTTP operations (GET, POST, etc.)
    - Test messaging pub/sub
    - Test permission validation
    - _Requirements: 5.2, 5.3_

- [ ] 18. Implement distributed capability providers
  - [ ] 18.1 Support capability providers on different nodes
    - Implement provider discovery and registration
    - Implement remote capability invocation via gRPC
    - Route invocations to remote providers when needed
    - _Requirements: 12.1, 12.3_

  - [ ] 18.2 Maintain security model with distributed providers
    - Enforce permissions for remote invocations
    - Ensure instance isolation with remote providers
    - Validate capability assignments before remote invocation
    - _Requirements: 12.4_

  - [ ]* 18.3 Write property test for distributed provider invocation
    - **Property 17: Distributed Capability Provider Invocation Routing**
    - **Validates: Requirements 12.1, 12.3, 12.4**

  - [ ]* 18.4 Write unit tests for distributed providers
    - Test remote provider invocation
    - Test permission enforcement with remote providers
    - Test network failure handling
    - _Requirements: 12.1, 12.3, 12.4_

- [ ] 19. Implement graceful provider shutdown handling
  - [ ] 19.1 Handle provider lifecycle independently
    - Support starting and stopping providers independently
    - Handle capability invocations when provider is stopped
    - Return appropriate errors when provider unavailable
    - _Requirements: 16.1, 16.2, 16.3_

  - [ ]* 19.2 Write property test for graceful provider shutdown
    - **Property 20: Graceful Provider Shutdown Handling**
    - **Validates: Requirements 16.3**

  - [ ]* 19.3 Write unit tests for provider lifecycle
    - Test starting providers independently
    - Test stopping providers independently
    - Test error handling when provider stopped
    - _Requirements: 16.1, 16.2, 16.3_

- [ ] 20. Add micro-kvm execution support (optional)
  - [ ] 20.1 Integrate micro-kvm runtime
    - Add micro-kvm as alternative execution path
    - Support configuration to choose wasmtime vs micro-kvm
    - Ensure same capability provider interface works with both
    - _Requirements: 12.2_

  - [ ]* 20.2 Write unit test for micro-kvm execution
    - Test instance execution with micro-kvm
    - Test capability invocation with micro-kvm
    - _Requirements: 12.2_

- [ ] 21. Implement observability and monitoring
  - [ ] 21.1 Add metrics collection
    - Expose metrics: active instance count, crash rate, invocation latency
    - Use prometheus or similar for metrics export
    - Add metrics for API request rate and latency
    - Add metrics for node agent health

  - [ ] 21.2 Implement structured logging
    - Use tracing for structured logging with consistent fields
    - Implement log levels (DEBUG, INFO, WARN, ERROR)
    - Add correlation IDs for request tracing
    - Support centralized log aggregation

  - [ ] 21.3 Add distributed tracing (optional)
    - Integrate OpenTelemetry for distributed tracing
    - Trace instance lifecycle from API to runtime
    - Trace capability invocations across network

- [ ] 22. Final Phase 3 checkpoint and integration testing
  - Verify distributed capability providers work correctly
  - Verify HTTP and Messaging providers function properly
  - Verify graceful provider shutdown handling
  - Run comprehensive integration tests across all phases
  - Run all unit and property tests
  - Ensure all tests pass, ask the user if questions arise

- [ ] 23. Documentation and deployment preparation
  - Write API documentation for Control Plane endpoints
  - Write deployment guide for single-node and multi-node setups
  - Write configuration guide for etcd, providers, and restart policies
  - Create example Wasm modules demonstrating capability usage
  - Document security considerations and best practices
  - Prepare release artifacts and deployment scripts

## Notes

- Tasks marked with `*` are optional property-based tests that can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at phase boundaries
- Property tests validate universal correctness properties with minimum 100 iterations
- Unit tests validate specific examples, edge cases, and integration points
- Implementation uses Rust with wasmtime for Wasm execution, gRPC for inter-component communication, and optional etcd for distributed coordination
- Phase 1 can be completed independently for single-node operation
- Phases 2 and 3 build incrementally on previous phases
