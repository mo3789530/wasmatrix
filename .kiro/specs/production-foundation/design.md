# Design Document: Production Foundation

## Overview

This design adds the minimum production foundation missing from the current implementation:

1. Durable metadata backed by a required persistent coordination store
2. Multi-control-plane high availability through leader election
3. A secure external REST API for third-party callers

It also defines the next operational layer:

4. Distributed tracing
5. Smarter scheduling
6. Quotas and execution limits

And the strategic differentiation layer:

7. Hardened capability sandboxing
8. Distributed metadata synchronization for edge/regional topologies
9. Policy-driven admission and authorization
10. Snapshot and migration workflows

The design keeps the current Rust workspace and existing control-plane / node-agent / provider split intact. Changes should extend the current structure rather than replace it.

## Architecture

### High-Level Target Architecture

```
┌──────────────────────────────┐
│ External Clients             │
│ REST + JWT / mTLS            │
└──────────────┬───────────────┘
               │
               v
┌──────────────────────────────┐
│ Control Plane Cluster        │
│ - REST API                   │
│ - gRPC services              │
│ - leader election            │
│ - durable metadata access    │
│ - policy / quota / tracing   │
└───────┬──────────────┬───────┘
        │              │
        v              v
┌───────────────┐  ┌──────────────────┐
│ Metadata Store│  │ Telemetry Backend│
│ etcd / FDB    │  │ OTLP / Prometheus│
└───────────────┘  └──────────────────┘
        │
        v
┌──────────────────────────────┐
│ Node Agents                  │
│ - runtime control            │
│ - trace propagation          │
│ - quota / sandbox checks     │
└──────────────┬───────────────┘
               │
               v
┌──────────────────────────────┐
│ Runtime + Capability Providers│
└──────────────────────────────┘
```

### Core Architectural Decisions

- `etcd` becomes the default and expected production coordination store.
- FoundationDB remains a future-compatible repository abstraction target, not the first implementation.
- Control Plane writes that mutate cluster state are leader-only.
- Read APIs may be served by followers if freshness rules are explicit, but phase 1 of this spec should keep reads leader-routed for simplicity.
- The external REST API is a wrapper over existing internal services, not a second business-logic implementation.
- Policy, quota, tracing, and sandboxing are cross-cutting concerns implemented in services, not controllers.

## Components and Interfaces

### 1. Durable Metadata Layer

Add a new `metadata_persistence` feature in `wasmatrix-control-plane`:

- `repo`
  - `MetadataRepository` trait
  - `EtcdMetadataRepository` as the initial required implementation
  - optional `FoundationDbMetadataRepository` placeholder trait adapter for future work
- `service`
  - orchestration of instance metadata persistence, crash history persistence, and cluster coordination records
- `controller`
  - thin integration surface for existing control-plane features

#### Persisted Record Classes

- `InstanceMetadata`
- capability assignments
- crash history
- control-plane leadership lease metadata
- node registration metadata
- provider metadata
- quota usage counters (if strongly consistent enforcement is required)

#### Storage Constraints

- Wasm memory is still not persisted.
- Desired state is still not introduced.
- Only system coordination metadata becomes durable.

### 2. Leader Election Layer

Add a new `leader_election` feature in `wasmatrix-control-plane`:

- `repo`
  - lease primitives backed by etcd
- `service`
  - acquire, renew, and release leader leases
  - expose local leadership state
- `controller`
  - read-only surface for main server/bootstrap wiring

#### Leadership Model

- Single active leader per cluster namespace
- Followers reject mutating API calls with redirect or retryable error
- Leadership renewal occurs on a bounded interval
- On lease expiration, the node self-demotes before serving mutating traffic

### 3. External REST API

Add a new `external_api` feature in `wasmatrix-control-plane`:

- `controller`
  - Axum handlers and DTO binding only
- `service`
  - authn/authz, request validation, idempotency checks, orchestration into existing instance and routing services
- `repo`
  - identity lookup, JWKS or key material access, optional RBAC policy storage

#### Initial REST Surface

```
POST   /v1/instances
GET    /v1/instances
GET    /v1/instances/{id}
POST   /v1/instances/{id}/stop
POST   /v1/instances/{id}/capabilities
DELETE /v1/instances/{id}/capabilities/{capability_id}
POST   /v1/capabilities/invoke
GET    /v1/healthz
GET    /v1/leader
```

#### Security Model

- Authentication:
  - JWT bearer tokens for service-to-service and external clients
  - optional mTLS for trusted internal callers
- Authorization:
  - RBAC roles such as `instance.admin`, `instance.read`, `capability.invoke`
- Audit:
  - all authenticated writes emit audit logs with subject, role set, trace ID, and target resource

### 4. Distributed Tracing

Add tracing support spanning:

- External REST request -> internal service -> gRPC call -> Node Agent -> capability provider

Implementation model:

- OpenTelemetry SDK in control plane and agent
- W3C trace context headers on HTTP
- gRPC metadata propagation for trace context
- span boundaries around:
  - API request
  - scheduling
  - leader election checks
  - metadata persistence
  - node RPC
  - provider invocation

### 5. Scheduler Strengthening

Extend `node_routing` service with a weighted scoring pipeline:

- resource score
- capability locality score
- health penalty
- latency score
- priority class multiplier

#### Scheduling Inputs

- node health
- current load
- supported capabilities
- provider locality
- historical latency measurements
- request priority class

### 6. Quotas and Limits

Add a new `quota_management` feature in `wasmatrix-control-plane`:

- `repo`
  - quota definitions and current usage
- `service`
  - admission checks and usage updates
- `controller`
  - internal integration only at first

Enforced scopes:

- global
- tenant
- namespace
- API identity

### 7. Hardened Capability Sandbox

Add runtime-enforced safeguards primarily in `wasmatrix-agent` and `wasmatrix-runtime`:

- invocation timeout budget
- rate limit window
- max invocation count per window
- memory guard configuration

Capability providers remain responsible for operation semantics, but not for policy ownership. Sandboxing policy is centrally evaluated before provider execution.

### 8. Distributed Metadata Synchronization

This is a later-stage optional extension over the durable metadata layer:

- strongly consistent metadata:
  - leadership
  - admission-critical quotas
  - active coordination locks
- eventually consistent metadata:
  - regional caches
  - derived scheduling hints
  - replicated status views

CRDT-backed structures should be limited to metadata classes that tolerate merge-based convergence.

### 9. Policy Engine

Add a `policy_engine` feature in `wasmatrix-control-plane`:

- `repo`
  - policy source loading
- `service`
  - evaluate instance start, capability assignment, and tenant isolation rules
- `controller`
  - thin internal adapter

The first version can use a simple internal DSL or JSON policy format. A later version may support OPA/Rego integration, but the service contract should abstract that away.

### 10. Snapshot and Migration

Add a new `migration` feature spanning control plane and node agent:

- snapshot request orchestration
- compatibility checks
- transfer coordination
- audit trail recording

Because current Wasm instances are designed to be stateless, this feature should initially target:

- metadata snapshots
- provider-bound external state references
- runtime-supported checkpoint hooks only where explicitly available

## Data Models

### Persistent Instance Record

```
{
  instance_id: string,
  node_id: string,
  module_hash: string,
  created_at: timestamp,
  status: enum,
  capabilities: [CapabilityAssignment],
  last_seen_at: timestamp,
  version: integer
}
```

### Crash History Record

```
{
  instance_id: string,
  crash_count: integer,
  last_crash_at: timestamp,
  last_error: optional<string>,
  backoff_until: optional<timestamp>
}
```

### Leader Lease Record

```
{
  cluster_id: string,
  leader_node_id: string,
  lease_id: string,
  acquired_at: timestamp,
  expires_at: timestamp
}
```

### External API Principal

```
{
  subject: string,
  authn_type: enum { jwt, mtls },
  roles: [string],
  tenant_id: optional<string>,
  expires_at: optional<timestamp>
}
```

### Quota Definition

```
{
  scope: string,
  max_instances: optional<integer>,
  max_capability_invocations_per_minute: optional<integer>,
  max_cpu_millis: optional<integer>,
  max_memory_bytes: optional<integer>
}
```

## Delivery Plan

### Phase A: Mandatory Production Baseline

- Durable metadata in etcd
- leader election
- external REST API with auth and RBAC

This phase is the minimum acceptable production baseline.

### Phase B: Operational Readiness

- distributed tracing
- stronger scheduler
- quotas and resource limits

### Phase C: Strategic Differentiation

- hardened capability sandbox
- distributed metadata synchronization
- policy engine
- snapshot and migration

## Risks and Constraints

- Requiring durable metadata changes the current "minimal state" stance; the implementation must keep stored data limited to platform coordination metadata only.
- Snapshot and migration are constrained by the current stateless design and may initially be metadata-oriented rather than full memory checkpointing.
- Strongly consistent quota enforcement can increase write amplification; not all quota counters should be globally serialized if local buffering is acceptable.
- Leader election must be integrated before exposing external write APIs in HA mode, otherwise split-brain writes remain possible.
