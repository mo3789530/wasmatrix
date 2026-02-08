# Design Document: Wasm Orchestrator

## Overview

The Wasm Orchestrator is a wasmCloud-based system for managing WebAssembly instance execution across distributed nodes. The design follows three core principles:

1. **Statelessness**: Wasm instances are ephemeral execution units with no trusted internal state
2. **Capability-based security**: All side effects are mediated through capability providers with explicit runtime permission enforcement
3. **Execution facts over desired state**: The orchestrator manages "what happened" rather than "what should be"

The system is designed for resilience through statelessness, treating instance crashes as acceptable events while ensuring node failures don't compromise the overall system.

## Architecture

### High-Level Architecture

```
┌─────────────────┐
│  Client / API   │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Control Plane  │
│  (Stateless)    │
└────────┬────────┘
         │
         v
┌─────────────────┐
│   Node Agent    │
│  (Per Node)     │
└────────┬────────┘
         │
         ├──────────────┐
         v              v
┌──────────────┐  ┌──────────────────┐
│ Wasm Runtime │  │ Capability       │
│              │  │ Providers        │
└──────────────┘  └──────────────────┘
```

### Component Interaction Flow

```
Client Request
    │
    v
Control Plane (validates, routes)
    │
    v
Node Agent (executes locally)
    │
    ├─> Wasm Runtime (runs instance)
    │
    └─> Capability Provider (handles side effects)
```

## Components and Interfaces

### Control Plane

**Responsibilities:**
- Receive and validate API requests
- Route instance start/stop requests to appropriate Node Agents
- Maintain in-memory cache of instance metadata and capability assignments
- Provide query interface for instance status

**State:**
- In-memory map: `instance_id -> InstanceMetadata`
- In-memory map: `instance_id -> [CapabilityAssignment]`
- No persistent application state

**Interface:**

```
API StartInstance(module_bytes, capabilities, restart_policy) -> instance_id
API StopInstance(instance_id) -> result
API QueryInstance(instance_id) -> InstanceStatus
API AssignCapability(instance_id, capability_id, permissions) -> result
API RevokeCapability(instance_id, capability_id) -> result
API ListInstances() -> [InstanceMetadata]
```

**InstanceMetadata:**
```
{
  instance_id: string,
  node_id: string,
  module_hash: string,
  created_at: timestamp,
  status: enum { starting, running, stopped, crashed }
}
```

**CapabilityAssignment:**
```
{
  instance_id: string,
  capability_id: string,
  provider_type: enum { kv, http, messaging },
  permissions: [string]
}
```

### Node Agent

**Responsibilities:**
- Manage local Wasm instance lifecycle
- Detect instance crashes and apply restart policies
- Communicate with Wasm Runtime for execution
- Report instance status to Control Plane
- Enforce capability provider access control

**State:**
- In-memory map: `instance_id -> RuntimeHandle`
- In-memory map: `instance_id -> RestartPolicy`
- Crash counters for backoff calculation

**Interface:**

```
Internal StartInstanceLocal(instance_id, module_bytes, capabilities) -> result
Internal StopInstanceLocal(instance_id) -> result
Internal RestartInstance(instance_id) -> result
Callback OnInstanceCrash(instance_id)
Internal GetInstanceStatus(instance_id) -> status
```

**RestartPolicy:**
```
{
  policy_type: enum { never, always, on_failure },
  max_retries: optional<int>,
  backoff_seconds: optional<int>
}
```

### Wasm Runtime

**Responsibilities:**
- Execute WebAssembly modules in isolated environments
- Provide WASI interface for Wasm instances
- Route capability invocations to appropriate providers
- Enforce memory isolation between instances

**Interface:**

```
Internal Execute(module_bytes, instance_id) -> RuntimeHandle
Internal Terminate(RuntimeHandle) -> result
Internal InvokeCapability(instance_id, capability_id, operation, params) -> result
```

### Capability Provider

**Responsibilities:**
- Provide specific side effect capabilities (KV, HTTP, messaging)
- Validate permissions before executing operations
- Execute operations on behalf of Wasm instances
- Return results or errors to calling instance

**Base Interface:**

```
Interface CapabilityProvider {
  Initialize(config) -> result
  Invoke(instance_id, operation, params) -> result
  Shutdown() -> result
  GetMetadata() -> ProviderMetadata
}
```

**KV Provider Operations:**
```
Get(key) -> optional<value>
Set(key, value) -> result
Delete(key) -> result
List(prefix) -> [key]
```

**HTTP Provider Operations:**
```
Request(method, url, headers, body) -> Response
```

**Messaging Provider Operations:**
```
Publish(topic, message) -> result
Subscribe(topic, callback) -> subscription_id
Unsubscribe(subscription_id) -> result
```

## Data Models

### Instance Lifecycle States

```
starting -> running -> stopped
              |
              v
           crashed -> (restart policy applies)
```

### Capability Permission Model

Permissions are string-based and provider-specific:

**KV Provider Permissions:**
- `kv:read` - Allow Get and List operations
- `kv:write` - Allow Set operations
- `kv:delete` - Allow Delete operations

**HTTP Provider Permissions:**
- `http:request` - Allow outbound HTTP requests
- `http:domain:<domain>` - Restrict requests to specific domain

**Messaging Provider Permissions:**
- `msg:publish:<topic>` - Allow publishing to specific topic
- `msg:subscribe:<topic>` - Allow subscribing to specific topic

### Communication Protocol

**Control Plane <-> Node Agent:**

Messages use a simple request-response protocol:

```
StartInstanceRequest {
  instance_id: string,
  module_bytes: bytes,
  capabilities: [CapabilityAssignment],
  restart_policy: RestartPolicy
}

StartInstanceResponse {
  success: bool,
  error: optional<string>
}

StopInstanceRequest {
  instance_id: string
}

StopInstanceResponse {
  success: bool,
  error: optional<string>
}

StatusReport {
  instance_id: string,
  status: InstanceStatus,
  timestamp: timestamp
}
```

**Wasm Instance <-> Capability Provider:**

Uses wasmCloud's standard capability invocation protocol:

```
CapabilityInvocation {
  instance_id: string,
  capability_id: string,
  operation: string,
  params: bytes (serialized)
}

CapabilityResponse {
  success: bool,
  result: optional<bytes>,
  error: optional<string>
}
```

## 
Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Before defining the correctness properties, let me analyze each acceptance criterion for testability.


### Property 1: Instance Statelessness Across Restarts

*For any* Wasm instance, when the instance is restarted or relocated to a different node, the orchestrator should not rely on or persist any internal memory state from the previous execution, and all persistent state should be externalized outside of instance memory.

**Validates: Requirements 1.1, 1.3, 1.5**

### Property 2: System State Preservation During Instance Crashes

*For any* Wasm instance crash, the orchestrator should preserve all system-level state (instance metadata, capability assignments, execution facts) while treating the crash as an acceptable event that doesn't compromise overall system operation.

**Validates: Requirements 1.2, 8.1, 8.3, 8.4**

### Property 3: Capability-Mediated Side Effects

*For any* I/O operation or OS resource access attempted by a Wasm instance, the operation must be routed through a capability provider, and direct OS access must be blocked, ensuring all side effects are mediated.

**Validates: Requirements 2.1, 2.2, 5.5**

### Property 4: Runtime Permission Enforcement

*For any* capability invocation by a Wasm instance, the orchestrator should verify that the instance has the required capability assignment and permissions before allowing the operation to proceed.

**Validates: Requirements 2.3, 2.5**

### Property 5: Capability Assignment Registry Completeness

*For any* Wasm instance with capability assignments, all assignments should be present in the orchestrator's registry and queryable.

**Validates: Requirements 2.4, 6.2**

### Property 6: Control Plane Instance Lifecycle Operations

*For any* valid start or stop request to the control plane, the corresponding instance should be created or terminated on an available node, and the instance metadata should be updated in the in-memory cache.

**Validates: Requirements 3.1, 3.2, 3.3**

### Property 7: Minimal State Storage Policy

*For any* operation performed by the orchestrator, the system should only store instance IDs and capability assignments, and should never persist application data, session state, business logic results, or execution logs.

**Validates: Requirements 6.1, 6.3, 6.4, 6.5, 6.6, 3.4**

### Property 8: Node Agent Crash Detection and Restart Policy Enforcement

*For any* Wasm instance crash detected by the node agent, the agent should apply the configured restart policy (never, always, or backoff) correctly for that specific instance.

**Validates: Requirements 4.1, 15.4, 15.5**

### Property 9: Node Agent Status Reporting

*For any* change in Wasm instance execution status, the node agent should report the updated status to the control plane.

**Validates: Requirements 4.5**

### Property 10: Capability Provider Invocation Execution

*For any* capability invocation from a Wasm instance, the capability provider should execute the operation on behalf of the instance and return the result or error.

**Validates: Requirements 5.4**

### Property 11: etcd Limited Usage When Enabled

*For any* orchestrator configuration where etcd is enabled, the system should store only node existence information and capability provider metadata in etcd, and should never store instance state, execution logs, or desired state in etcd.

**Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5**

### Property 12: Node Failure Resilience

*For any* node failure in a multi-node deployment, the orchestrator should remain operational on other nodes without losing system-level state.

**Validates: Requirements 8.2**

### Property 13: Execution Facts Recording

*For any* execution event that occurs, the orchestrator should record the event as an execution fact without performing desired state reconciliation or maintaining desired state specifications.

**Validates: Requirements 9.1, 9.2, 9.3**

### Property 14: Actual Status Reporting

*For any* status query to the orchestrator, the response should reflect the actual current execution status rather than any intended or desired status.

**Validates: Requirements 9.4**

### Property 15: In-Memory Instance Management

*For any* instance management operation in Phase 1, the orchestrator should use in-memory data structures without requiring etcd.

**Validates: Requirements 10.2, 10.4**

### Property 16: Control Plane and Node Agent Protocol Communication

*For any* message exchanged between the control plane and node agent, the message should conform to the defined protocol format (StartInstanceRequest/Response, StopInstanceRequest/Response, StatusReport).

**Validates: Requirements 11.3**

### Property 17: Distributed Capability Provider Invocation Routing

*For any* capability invocation where the provider is running on a different node than the Wasm instance, the orchestrator should correctly route the invocation across the network while maintaining the same security model and permission enforcement.

**Validates: Requirements 12.1, 12.3, 12.4**

### Property 18: API Request Validation

*For any* API request received by the control plane, the request should be validated before processing, and any validation or processing failures should return descriptive error information.

**Validates: Requirements 13.5, 13.6**

### Property 19: Instance Isolation Enforcement

*For any* pair of Wasm instances running on the same node, each instance should be isolated such that neither can access the other's memory, capability assignments, or capability provider access, with all access scoped to the requesting instance.

**Validates: Requirements 14.1, 14.2, 14.3, 14.4, 14.5**

### Property 20: Graceful Provider Shutdown Handling

*For any* capability provider that is stopped, subsequent capability invocations from instances should be handled gracefully with appropriate error responses rather than system crashes.

**Validates: Requirements 16.3**

### Property 21: Provider and Instance Metadata Separation

*For any* capability provider and instance metadata stored by the orchestrator, the metadata should be maintained in separate data structures.

**Validates: Requirements 16.5**

## Error Handling

### Error Categories

**1. Instance Lifecycle Errors:**
- Module loading failures (invalid Wasm bytecode)
- Instance startup failures (resource exhaustion)
- Instance crash detection
- Instance termination failures

**Error Handling Strategy:**
- Log error details with instance_id and timestamp
- Return descriptive error to API caller
- Update instance status to reflect error state
- Apply restart policy if applicable
- Never propagate instance errors to other instances

**2. Capability Invocation Errors:**
- Permission denied (missing capability assignment)
- Provider unavailable (stopped or crashed)
- Operation timeout
- Invalid parameters
- Provider-specific errors (e.g., key not found in KV)

**Error Handling Strategy:**
- Return error to calling Wasm instance
- Log error with instance_id, capability_id, and operation
- Do not restart instance on capability errors
- Maintain capability provider availability status
- Provide clear error messages for debugging

**3. Communication Errors:**
- Control Plane <-> Node Agent communication failure
- Node Agent <-> Wasm Runtime communication failure
- Wasm Instance <-> Capability Provider communication failure
- Network timeouts in distributed mode

**Error Handling Strategy:**
- Implement retry logic with exponential backoff
- Log communication failures with source and destination
- Mark nodes as unavailable after repeated failures
- Provide fallback behavior where possible
- Alert operators on persistent communication issues

**4. Resource Exhaustion Errors:**
- Out of memory on node
- Too many instances on node
- Capability provider resource limits
- Network bandwidth exhaustion

**Error Handling Strategy:**
- Reject new instance creation with clear error
- Implement resource quotas per node
- Monitor resource usage and provide metrics
- Support graceful degradation
- Allow operators to configure resource limits

**5. Configuration Errors:**
- Invalid restart policy
- Invalid capability permissions
- Missing required configuration
- Incompatible configuration combinations

**Error Handling Strategy:**
- Validate configuration at API entry point
- Return validation errors before processing
- Provide default values where appropriate
- Document configuration requirements
- Fail fast on invalid configuration

### Error Response Format

All API errors follow a consistent format:

```
ErrorResponse {
  error_code: string,
  message: string,
  details: optional<map<string, string>>,
  timestamp: timestamp
}
```

**Error Codes:**
- `INVALID_REQUEST` - Malformed or invalid API request
- `INSTANCE_NOT_FOUND` - Specified instance does not exist
- `PERMISSION_DENIED` - Capability permission check failed
- `RESOURCE_EXHAUSTED` - Node resources unavailable
- `PROVIDER_UNAVAILABLE` - Capability provider not running
- `COMMUNICATION_FAILURE` - Inter-component communication failed
- `INTERNAL_ERROR` - Unexpected system error

### Crash Recovery

**Instance Crash Recovery:**
1. Node Agent detects crash via runtime callback
2. Log crash event with instance_id and timestamp
3. Evaluate restart policy for instance
4. If restart: clean up old runtime handle, start new instance
5. If no restart: update status to crashed, notify control plane
6. Preserve all system-level state throughout

**Node Crash Recovery:**
1. Control Plane detects node unavailability via heartbeat timeout
2. Mark all instances on node as unavailable
3. In Phase 2+: redistribute instances to other nodes (optional)
4. Log node failure event
5. When node recovers: re-register with control plane
6. Rebuild node agent state from control plane

**Control Plane Crash Recovery:**
1. On restart: initialize empty in-memory cache
2. Query all node agents for current instance status
3. Rebuild instance metadata from status reports
4. Rebuild capability assignments from node agent state
5. Resume normal operation
6. No desired state to reconcile - only rebuild facts

## Testing Strategy

### Dual Testing Approach

The Wasm Orchestrator will use both unit tests and property-based tests to ensure comprehensive coverage:

**Unit Tests** focus on:
- Specific examples of API operations (start, stop, query)
- Edge cases (empty requests, invalid IDs, malformed data)
- Error conditions (resource exhaustion, permission denied)
- Integration points between components
- Specific deployment scenarios (single-node, multi-node, with/without etcd)

**Property-Based Tests** focus on:
- Universal properties that hold for all inputs
- Statelessness guarantees across random instance lifecycles
- Permission enforcement across random capability assignments
- Isolation across random instance combinations
- Crash resilience across random failure scenarios

Together, unit tests catch concrete bugs in specific scenarios, while property tests verify general correctness across the input space.

### Property-Based Testing Configuration

**Testing Library:** We will use a property-based testing library appropriate for the implementation language (e.g., Hypothesis for Python, fast-check for TypeScript/JavaScript, QuickCheck for Haskell, PropCheck for Rust).

**Test Configuration:**
- Minimum 100 iterations per property test (due to randomization)
- Each property test references its design document property
- Tag format: `Feature: wasm-orchestrator, Property {number}: {property_text}`

**Example Property Test Structure:**

```
test_property_1_instance_statelessness:
  # Feature: wasm-orchestrator, Property 1: Instance Statelessness Across Restarts
  for 100 iterations:
    instance = generate_random_instance()
    initial_state = capture_system_state()
    
    start_instance(instance)
    set_instance_memory(instance, random_data())
    restart_instance(instance)
    
    assert orchestrator_does_not_use_old_memory(instance)
    assert system_state_unchanged(initial_state)
```

### Unit Test Coverage

**Component-Level Tests:**

1. **Control Plane Tests:**
   - Test start API with valid module
   - Test stop API with existing instance
   - Test query API with various instance states
   - Test capability assignment API
   - Test error responses for invalid requests
   - Test in-memory cache operations

2. **Node Agent Tests:**
   - Test local instance start/stop
   - Test crash detection callback
   - Test restart policy enforcement (never, always, backoff)
   - Test status reporting to control plane
   - Test runtime communication

3. **Capability Provider Tests:**
   - Test KV provider operations (get, set, delete, list)
   - Test HTTP provider request handling
   - Test messaging provider pub/sub
   - Test permission validation
   - Test error handling for invalid operations

4. **Integration Tests:**
   - Test end-to-end instance lifecycle (start -> run -> stop)
   - Test capability invocation from instance through provider
   - Test multi-node deployment with etcd
   - Test node failure and recovery
   - Test control plane restart and state rebuild

### Test Data Generation

For property-based tests, we need generators for:

**Instance Generators:**
- Random valid Wasm modules
- Random instance IDs
- Random restart policies
- Random capability assignments

**Capability Generators:**
- Random KV operations (keys, values)
- Random HTTP requests (methods, URLs, headers)
- Random messaging operations (topics, messages)
- Random permission sets

**Failure Generators:**
- Random instance crashes
- Random node failures
- Random network partitions
- Random resource exhaustion scenarios

### Phase-Specific Testing

**Phase 1 Testing:**
- Focus on single-node operation
- Test in-memory state management
- Test basic KV provider
- No etcd testing required

**Phase 2 Testing:**
- Add multi-node tests
- Test control plane/node agent separation
- Test etcd integration (when enabled)
- Test distributed coordination

**Phase 3 Testing:**
- Test distributed capability providers
- Test micro-kvm execution path
- Test cross-node capability invocation
- Test advanced failure scenarios

### Continuous Testing

- Run unit tests on every commit
- Run property tests nightly (due to longer execution time)
- Monitor test coverage and aim for >80% code coverage
- Track property test failure rates and investigate anomalies
- Use test results to identify edge cases for additional unit tests

## Implementation Notes

### Phase 1 Implementation Priority

1. Define core data structures (InstanceMetadata, CapabilityAssignment, RestartPolicy)
2. Implement Control Plane API handlers (start, stop, query)
3. Implement Node Agent with basic runtime integration
4. Implement KV capability provider
5. Implement crash detection and restart logic
6. Add comprehensive testing

### Phase 2 Considerations

- Design clean separation between Control Plane and Node Agent
- Define communication protocol with versioning support
- Implement etcd integration with minimal scope
- Add multi-node coordination logic
- Test distributed scenarios thoroughly

### Phase 3 Considerations

- Design remote capability invocation protocol
- Implement capability provider discovery and routing
- Add micro-kvm runtime support
- Optimize network communication for distributed providers
- Consider security implications of distributed architecture

### Technology Choices

**Wasm Runtime Options:**
- wasmtime (Rust-based, production-ready)
- wasmer (Rust-based, good performance)
- wazero (Go-based, zero dependencies)

**Communication Protocol:**
- gRPC for Control Plane <-> Node Agent (typed, efficient)
- wasmCloud RPC for Wasm <-> Capability Provider (standard)

**Serialization:**
- Protocol Buffers for structured messages
- MessagePack for capability invocations (compact)

**Optional etcd Usage:**
- etcd v3 API
- Lease-based node registration
- Watch-based change notification

### Security Considerations

1. **Wasm Sandbox:** Rely on Wasm runtime's built-in sandboxing
2. **Capability Permissions:** Enforce at invocation time, not just assignment time
3. **API Authentication:** Add authentication layer for control plane API (Phase 2+)
4. **Network Security:** Use TLS for inter-component communication (Phase 2+)
5. **Resource Limits:** Enforce memory and CPU limits per instance
6. **Audit Logging:** Log all security-relevant events (capability grants, permission denials)

### Performance Considerations

1. **Instance Startup:** Optimize module loading and compilation
2. **Capability Invocation:** Minimize serialization overhead
3. **State Queries:** Use efficient in-memory data structures (hash maps)
4. **Crash Detection:** Use async callbacks to avoid blocking
5. **Network Communication:** Use connection pooling and multiplexing
6. **Monitoring:** Expose metrics for instance count, crash rate, invocation latency

### Observability

**Metrics to Expose:**
- Active instance count per node
- Instance crash rate
- Capability invocation rate and latency
- API request rate and latency
- Node agent health status
- Capability provider availability

**Logging Strategy:**
- Structured logging with consistent fields (instance_id, timestamp, component)
- Log levels: DEBUG (detailed), INFO (lifecycle events), WARN (recoverable errors), ERROR (failures)
- Centralized log aggregation in distributed mode
- Correlation IDs for request tracing

**Tracing:**
- Distributed tracing for multi-component operations
- Trace instance lifecycle from API request to runtime execution
- Trace capability invocations across network boundaries
- Use OpenTelemetry for standardization
