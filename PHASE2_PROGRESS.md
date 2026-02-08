# Phase 2 Progress Report

## Completed Tasks

### 12.1 Define communication protocol
- Created `crates/wasmatrix-proto/proto/wasmatrix.proto` defining `NodeAgentService` and `ControlPlaneService`.
- Configured `tonic` and `prost` for gRPC code generation.
- Implemented `crates/wasmatrix-proto/src/conversion.rs` for type conversion between `v1` (gRPC) and `protocol` (internal) types.

### 12.3 Implement gRPC service
- Implemented `ControlPlaneServer` in `crates/wasmatrix-control-plane/src/server.rs`.
- Implemented `NodeAgentServer` in `crates/wasmatrix-agent/src/server.rs`.
- Validated that all crates compile with the new gRPC infrastructure.

### 12.5 Update Node Agent to report status to Control Plane
- Added `status_reporting` feature under `crates/wasmatrix-agent/src/features/status_reporting/`.
- Implemented controller/service/repo split:
  - `controller`: periodic reporting scheduler and status-change trigger methods
  - `service`: builds heartbeat/status-change reports
  - `repo`: gRPC client to `ControlPlaneService.ReportStatus`
- Updated `crates/wasmatrix-agent/src/main.rs` to:
  - start gRPC `NodeAgentServiceServer`
  - connect to control plane and send initial + periodic heartbeats
- Updated `crates/wasmatrix-agent/src/server.rs` to report immediate status changes on instance start/stop.

### 12.4 Update Control Plane to route requests to Node Agents
- Added `node_routing` feature under `crates/wasmatrix-control-plane/src/features/node_routing/`.
- Implemented controller/service/repo split:
  - `repo`: in-memory node registry and instance placement map
  - `service`: start/stop/query/list routing to `NodeAgentService` and node availability handling
  - `controller`: thin wrapper for register/report/route operations
- Updated `crates/wasmatrix-control-plane/src/main.rs` to start gRPC `ControlPlaneServiceServer`.
- Updated `crates/wasmatrix-control-plane/src/server.rs`:
  - `register_node` persists node registry entries through `NodeRoutingController`
  - `report_status` records heartbeat and updates control-plane instance status from node reports

### 12.2 / 12.6 Property Tests for Distributed Communication
- Added Property 16 protocol communication tests in `crates/wasmatrix-proto/src/protocol_tests.rs`.
- Added Property 9 status reporting test in `crates/wasmatrix-control-plane/src/server.rs`.
- Verified protocol conversion round-trip and report-driven status transitions across distributed interfaces.

### 12.7 Unit Tests for Distributed Communication
- Added unit tests for distributed messaging and validation in `crates/wasmatrix-control-plane/src/server.rs`.
- Added unit tests for multi-node routing selection behavior in `crates/wasmatrix-control-plane/src/features/node_routing/service/mod.rs`.

### 13 Optional etcd Integration
- Added optional `etcd` feature and `etcd-client` dependency wiring in control-plane/workspace Cargo files.
- Added `EtcdConfig` env-based configuration in `crates/wasmatrix-control-plane/src/features/node_routing/repo/etcd.rs`.
- Added metadata storage separation path (`ProviderMetadata`) in node routing repository as groundwork for limited etcd usage.
- Added `EtcdMetadataRepository` with strict key scoping (nodes/providers only), and wired node/provider metadata writes from `NodeRoutingService`.
- Added property/unit tests for etcd metadata constraints and integration behavior.

### 14 Multi-node Support
- Added static node discovery bootstrap via `STATIC_NODE_AGENTS` in control-plane startup.
- Enhanced routing selection to be capability-aware and least-loaded across registered nodes.
- Added node failure resilience property tests and multi-node routing unit tests.

### 15 Control Plane Crash Recovery
- Added `ControlPlane::restore_instance_state` to restore recovered instance metadata and capabilities.
- Extended node routing repo/service/controller with recovery flow that:
  - queries node-local state from `NodeAgentService.ListInstances`
  - reapplies recovered instances into control-plane state
  - rebuilds node assignment and active-instance counters
- Updated control-plane `register_node` path to trigger recovery from the newly registered node.
- Added unit test `test_recover_node_state_applies_instance_statuses` for recovery application behavior.

## Next Steps

1. **Task 16**: Phase 2 checkpoint verification.
2. **Task 17.1**: Implement HTTP capability provider.
3. **Task 17.2**: Implement Messaging capability provider.
