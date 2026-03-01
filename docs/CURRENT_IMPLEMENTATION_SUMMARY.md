# Current Implementation Summary

## Overview

As of March 1, 2026, this repository is a Rust workspace implementing the core parts of a Wasm orchestrator with a distributed control plane, node agent, capability providers, protocol definitions, and a lightweight runtime abstraction.

The current implementation is centered on internal gRPC APIs plus a metrics HTTP endpoint. There is not yet a general-purpose external REST wrapper API for third-party callers.

## Implemented Crates

### `wasmatrix-core`

- Core domain types:
  - `InstanceMetadata`
  - `CapabilityAssignment`
  - `RestartPolicy`
  - `InstanceStatus`
  - `ErrorResponse`
- Capability and permission model:
  - provider types: `Kv`, `Http`, `Messaging`
  - permission helpers and enforcement primitives
- Execution facts model:
  - in-memory `ExecutionEventRecorder`
  - records start, stop, crash, and restart events
- Statelessness and isolation support modules:
  - `statelessness`
  - `isolation`

### `wasmatrix-control-plane`

- In-memory control plane state management:
  - start instance
  - stop instance
  - query instance
  - list instances
  - assign capability
  - revoke capability
  - restore recovered instance state
  - update instance status
  - crash tracking and recovery bookkeeping
- gRPC Control Plane service:
  - `RegisterNode`
  - `ReportStatus`
- Feature-sliced modules:
  - `instance_management`
    - controller/service/repo split for instance lifecycle operations
  - `leader_election`
    - controller/service/repo split for control-plane lease ownership
    - background leadership acquisition and renewal
    - follower rejection for mutating control-plane gRPC APIs
  - `node_routing`
    - node registration and node selection
    - routing start/stop/query/list requests to node agents
    - distributed capability invocation routing
    - optional etcd-backed metadata integration
  - `observability`
    - Prometheus metric recording and rendering
- HTTP endpoint:
  - `/metrics` via Axum
- Structured logging:
  - `x-correlation-id` extraction with UUID fallback

### `wasmatrix-agent`

- Local Wasm instance lifecycle management:
  - start local instance
  - stop local instance
  - query local status
  - list local instances
- Crash handling:
  - crash detection callback path
  - crash counters
  - exponential backoff calculation
  - restart-policy evaluation (`never`, `always`, `on_failure`)
- gRPC Node Agent service:
  - `StartInstance`
  - `StopInstance`
  - `QueryInstance`
  - `ListInstances`
  - `InvokeCapability`
- Status reporting feature:
  - controller/service/repo split for reporting status back to control plane
- Provider lifecycle integration:
  - start provider
  - stop provider
  - reject invocations when provider is stopped

### `wasmatrix-providers`

- Capability provider trait:
  - `initialize`
  - `invoke`
  - `shutdown`
  - `get_metadata`
- Implemented providers:
  - KV provider
    - `get`
    - `set`
    - `delete`
    - `list`
  - HTTP capability provider
    - outbound HTTP requests via reqwest
    - method validation
    - permission checks:
      - `http:request`
      - optional host-scoped `http:domain:<host>`
  - Messaging capability provider
    - in-memory publish
    - subscribe
    - unsubscribe
    - permission checks:
      - `msg:publish`
      - `msg:publish:<topic>`
      - `msg:subscribe`
      - `msg:subscribe:<topic>`
- Provider lifecycle feature:
  - independent running/stopped state management

### `wasmatrix-runtime`

- Runtime abstraction with selectable backend:
  - `Wasmtime`
  - `MicroKvm`
- Environment-based backend selection:
  - `WASM_RUNTIME_BACKEND`
- Shared capability invocation path through `CapabilityManager`
- Built-in registration of a default KV provider
- Basic module validation before execution

### `wasmatrix-proto`

- Protocol types for control plane <-> node agent communication
- Protobuf definitions and generated gRPC bindings (`tonic`)
- Conversion layer between protobuf and core/protocol types
- Message coverage includes:
  - instance lifecycle requests/responses
  - capability invocation requests/responses
  - node registration
  - status reporting

## Currently Exposed Interfaces

### Internal gRPC

- Control Plane service for node agents
  - node registration
  - status reports
- Node Agent service for control plane
  - instance lifecycle control
  - instance queries
  - capability invocation

### Internal HTTP

- Metrics endpoint:
  - `GET /metrics`
- Control-plane diagnostics:
  - `GET /healthz`
  - `GET /leader`

## Implementation Status Notes

- Distributed capability invocation is implemented for KV, HTTP, and Messaging providers.
- Leader election is implemented with an in-memory default and an etcd-backed repository behind the `etcd` feature flag.
- Provider lifecycle start/stop handling is implemented.
- Prometheus metrics and structured logs are implemented.
- The legacy Elixir scaffold was removed from the repository.

## Not Yet Implemented or Still Incomplete

- External-facing wrapper REST API for third-party callers
- Distributed tracing (`21.3` remains pending)
- Full documentation and deployment preparation task set (`23` remains in progress)
- Some optional property-test tasks in the implementation plan remain unimplemented
