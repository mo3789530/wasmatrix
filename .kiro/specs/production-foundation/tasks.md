# Implementation Plan: Production Foundation

## Overview

This plan converts the production foundation design into prioritized implementation work. The work is split into three tiers:

- `P0`: mandatory production baseline
- `P1`: operational readiness
- `P2`: strategic differentiation

The first release target for this spec is completing all `P0` tasks.

## Tasks

- [~] P0-1. Introduce durable metadata persistence as a required production dependency
  - Create `metadata_persistence` feature in `crates/wasmatrix-control-plane/src/features/`
  - Define `MetadataRepository` trait with records for:
    - instance metadata
    - capability assignments
    - crash history
    - node metadata
    - provider metadata
  - Promote etcd-backed implementation from optional metadata helper to the primary production repository
  - Remove production code paths that assume in-memory-only authoritative state
  - Implementation: Added initial `metadata_persistence` feature with `PersistentMetadataRepository` and `EtcdBackedMetadataRepository`
  - Implementation: Added durable storage for `InstanceMetadata` and crash history records
  - Implementation: Added `etcd-client` backed read/write code paths behind `--features etcd`
  - Note: local `--features etcd` build is currently blocked by missing `protoc` in the environment
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [~] P0-2. Persist instance metadata and crash history in control-plane services
  - Wire `instance_management` writes through the durable repository
  - Persist crash bookkeeping on status updates and recovery handling
  - Load durable records during control-plane startup
  - Keep in-memory caches as read-through or write-through accelerators only
  - Implementation: Added `PersistentInstanceRepository` adapter and `InstanceService::new_with_persistence`
  - Implementation: `create`, `get`, and `update_status(Crashed)` now sync durable metadata when using the persistence-backed service
  - Implementation: Wired `ControlPlaneServer` status-report processing to persist status transitions and crash history when metadata persistence is enabled
  - Implementation: Wired `main.rs` bootstrap to create and pass a metadata persistence controller alongside etcd-enabled startup
  - _Requirements: 1.2, 1.3, 1.4_

- [~] P0-3. Implement leader election for multi-control-plane deployment
  - Create `leader_election` feature in `crates/wasmatrix-control-plane/src/features/`
  - Implement etcd lease acquisition, renewal, and release
  - Add leadership gating for all mutating APIs
  - Expose local leadership status for health and diagnostics
  - Implementation: Added `leader_election` feature with repo/service/controller layers and background lease acquisition/renewal
  - Implementation: Added in-memory leader election by default plus `EtcdLeaderElectionRepository` behind `--features etcd`
  - Implementation: Gated mutating gRPC APIs (`RegisterNode`, `ReportStatus`) so follower nodes reject writes with `FAILED_PRECONDITION`
  - Implementation: Added `/healthz` and `/leader` HTTP diagnostics backed by local leadership state
  - Note: local validation covered default build/test paths; the `--features etcd` code path was not exercised in this environment
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_

- [ ] P0-4. Add an external authenticated REST API
  - Create `external_api` feature in `crates/wasmatrix-control-plane/src/features/`
  - Add Axum routes for:
    - `POST /v1/instances`
    - `GET /v1/instances`
    - `GET /v1/instances/{id}`
    - `POST /v1/instances/{id}/stop`
    - `POST /v1/instances/{id}/capabilities`
    - `DELETE /v1/instances/{id}/capabilities/{capability_id}`
    - `POST /v1/capabilities/invoke`
  - Reuse existing internal services rather than duplicating orchestration logic
  - _Requirements: 3.1, 3.2, 3.5_

- [ ] P0-5. Add external API authentication and RBAC
  - Implement JWT validation and principal extraction
  - Add optional mTLS identity mapping for trusted callers
  - Implement RBAC role checks in the external API service layer
  - Add audit logging for authenticated write paths
  - _Requirements: 3.3, 3.4_

- [ ] P0-6. Add production configuration and bootstrap rules
  - Require persistent metadata configuration in production mode
  - Add explicit startup failure when durable store is unavailable
  - Add config for leader election timing, REST bind address, auth settings, and TLS materials
  - Document local-dev mode vs production mode behavior
  - _Requirements: 1.1, 2.4, 3.3_

- [ ] P0-7. Add integration tests for HA baseline
  - Test control-plane restart with durable metadata recovery
  - Test leader failover and follower write rejection
  - Test authenticated REST instance lifecycle flows
  - Test unauthorized and forbidden access paths
  - _Requirements: 1.4, 2.3, 3.1, 3.3, 3.4_

- [ ] P0-8. Checkpoint: validate the production baseline
  - Run workspace tests
  - Run workspace build
  - Verify that `P0-1` through `P0-7` are complete before marking the baseline production-ready

- [ ] P1-1. Implement distributed tracing across HTTP, gRPC, and provider invocation
  - Add OpenTelemetry setup to control plane and node agent
  - Propagate W3C trace context on REST
  - Propagate trace metadata over gRPC
  - Add spans for scheduling, persistence, node RPC, and capability invocation
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [ ] P1-2. Strengthen the scheduler scoring model
  - Extend `node_routing` with weighted scoring inputs
  - Add capability locality preference
  - Penalize degraded nodes
  - Add latency-aware routing inputs
  - Add priority classes
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

- [ ] P1-3. Implement quotas and resource limits
  - Create `quota_management` feature in `crates/wasmatrix-control-plane/src/features/`
  - Enforce instance-count quotas
  - Enforce capability invocation quotas
  - Add CPU and memory limit enforcement hooks where supported
  - Export quota usage metrics
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [ ] P1-4. Add operational test coverage for tracing, scheduler behavior, and quota enforcement
  - Validate trace propagation end to end
  - Validate placement scoring decisions
  - Validate quota rejection behavior
  - _Requirements: 4.5, 5.5, 6.4_

- [ ] P1-5. Checkpoint: validate operational readiness
  - Run workspace tests
  - Run workspace build
  - Confirm `P1-1` through `P1-4` are complete

- [ ] P2-1. Harden capability sandbox enforcement
  - Add timeout budgets for capability invocations
  - Add per-capability and per-instance rate limiting
  - Add invocation-count window limits
  - Add memory guard enforcement hooks in runtime and agent
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [ ] P2-2. Add distributed metadata synchronization for edge and regional topologies
  - Define strongly consistent vs eventually consistent metadata classes
  - Add CRDT-oriented replication for eligible metadata
  - Add regional cache and merge semantics
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [ ] P2-3. Implement a pluggable policy engine
  - Create `policy_engine` feature in `crates/wasmatrix-control-plane/src/features/`
  - Evaluate policies on instance start and capability assignment changes
  - Add tenant isolation rules
  - Return descriptive denial reasons
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

- [ ] P2-4. Define snapshot and migration workflows
  - Create migration orchestration contracts between control plane and node agent
  - Add compatibility validation before migration
  - Record snapshot and migration audit events
  - Start with metadata-oriented snapshots where full checkpointing is unavailable
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] P2-5. Add strategic test coverage for sandboxing, synchronization, policy, and migration
  - Validate limit violation handling
  - Validate merge behavior for replicated metadata
  - Validate policy denial paths
  - Validate migration preflight checks
  - _Requirements: 7.5, 8.3, 9.4, 10.4_

- [ ] P2-6. Checkpoint: validate strategic feature readiness
  - Run workspace tests
  - Run workspace build
  - Confirm `P2-1` through `P2-5` are complete

## Sequencing Notes

- `P0-1` through `P0-3` must land before the external REST API is considered safe for HA production.
- `P0-4` and `P0-5` should be implemented together to avoid exposing unauthenticated write paths.
- `P1-3` quota enforcement should integrate with `P0` durable metadata decisions to avoid duplicate storage abstractions.
- `P2-4` should be delayed until the policy and sandbox boundaries are stable, because migration depends on both.
