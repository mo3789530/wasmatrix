# Requirements Document

## Introduction

The Wasm Orchestrator is a wasmCloud-based system for managing WebAssembly instance execution across distributed nodes. The system follows the wasmCloud philosophy of "unbreakable, unrestricted, stateless" execution, where Wasm instances are treated as ephemeral execution units that can be restarted or relocated at any time. The orchestrator focuses on execution control rather than desired state reconciliation, managing "events that happened" rather than maintaining application state. All side effects are mediated through capability providers, ensuring security through explicit permission grants enforced at runtime.

## Glossary

- **Wasm_Instance**: A single WebAssembly module execution unit that is stateless and restart-assumed
- **Control_Plane**: The stateless API layer responsible for receiving requests and managing instance lifecycle and capability assignments
- **Node_Agent**: The local execution manager responsible for running Wasm instances on a single node and handling crash detection
- **Capability_Provider**: The exclusive mechanism through which Wasm instances can cause side effects (e.g., KV storage, HTTP, messaging)
- **Wasm_Runtime**: The WebAssembly execution environment managed by the Node Agent
- **Instance_ID**: A unique identifier for a Wasm instance managed by the orchestrator
- **Capability_Assignment**: The explicit grant of permissions linking a Wasm instance to a capability provider
- **Execution_Facts**: Historical records of what happened during execution, as opposed to desired state

## Requirements

### Requirement 1: Stateless Wasm Instance Management

**User Story:** As a system architect, I want Wasm instances to be treated as stateless execution units, so that the system remains resilient to instance crashes and relocations.

#### Acceptance Criteria

1. WHEN a Wasm instance is restarted, THE Orchestrator SHALL NOT rely on any in-memory state from the previous execution
2. WHEN a Wasm instance crashes, THE Orchestrator SHALL be capable of restarting it without data loss to the system
3. THE Orchestrator SHALL NOT persist Wasm instance internal memory state
4. WHEN a Wasm instance is relocated to a different node, THE Orchestrator SHALL ensure the instance operates correctly without state migration
5. THE Orchestrator SHALL externalize all persistent state outside of Wasm instance memory

### Requirement 2: Capability-Based Side Effect Control

**User Story:** As a security engineer, I want all Wasm side effects to be mediated through capability providers, so that Wasm instances cannot directly access OS resources.

#### Acceptance Criteria

1. WHEN a Wasm instance attempts to perform I/O operations, THE Orchestrator SHALL enforce that all operations go through capability providers
2. THE Orchestrator SHALL prevent Wasm instances from directly accessing operating system resources
3. WHEN a capability is assigned to a Wasm instance, THE Orchestrator SHALL enforce the permission grant at runtime
4. THE Orchestrator SHALL maintain a registry of capability assignments for each Wasm instance
5. WHEN a Wasm instance is started without required capability assignments, THE Orchestrator SHALL prevent operations requiring those capabilities

### Requirement 3: Control Plane Instance Lifecycle Management

**User Story:** As an operator, I want the control plane to manage Wasm instance start and stop requests, so that I can control execution across the cluster.

#### Acceptance Criteria

1. WHEN a start request is received, THE Control_Plane SHALL initiate Wasm instance creation on an available node
2. WHEN a stop request is received, THE Control_Plane SHALL terminate the specified Wasm instance
3. THE Control_Plane SHALL maintain an in-memory cache of active instance metadata
4. THE Control_Plane SHALL NOT persist application state or business logic results
5. WHEN the Control Plane restarts, THE Control_Plane SHALL rebuild its state from execution facts rather than stored desired state

### Requirement 4: Node Agent Local Execution Management

**User Story:** As a system operator, I want the node agent to manage local Wasm instance execution, so that instances run reliably on individual nodes.

#### Acceptance Criteria

1. WHEN a Wasm instance crashes, THE Node_Agent SHALL detect the crash and optionally restart the instance
2. THE Node_Agent SHALL communicate directly with the Wasm runtime for instance lifecycle operations
3. THE Node_Agent SHALL operate independently on a single node without requiring cluster coordination
4. WHEN multiple Node Agents are deployed, THE Node_Agent SHALL support distributed operation mode
5. THE Node_Agent SHALL report instance execution status to the Control Plane

### Requirement 5: Capability Provider Integration

**User Story:** As a developer, I want Wasm instances to interact with external resources through capability providers, so that side effects are controlled and secure.

#### Acceptance Criteria

1. THE Orchestrator SHALL support KV (key-value) capability providers for data storage
2. THE Orchestrator SHALL support HTTP capability providers for network communication
3. THE Orchestrator SHALL support messaging capability providers for asynchronous communication
4. WHEN a Wasm instance invokes a capability, THE Capability_Provider SHALL execute the operation on behalf of the instance
5. THE Orchestrator SHALL ensure Wasm instances cannot bypass capability providers to access resources directly

### Requirement 6: Minimal State Management

**User Story:** As a system architect, I want the orchestrator to hold minimal state, so that the system remains simple and resilient.

#### Acceptance Criteria

1. THE Orchestrator SHALL store Instance IDs for active Wasm instances
2. THE Orchestrator SHALL store capability assignment information
3. THE Orchestrator SHALL NOT store application data
4. THE Orchestrator SHALL NOT store session state
5. THE Orchestrator SHALL NOT store business logic results
6. THE Orchestrator SHALL NOT store execution logs as persistent state

### Requirement 7: Optional etcd Integration

**User Story:** As a system architect, I want optional etcd integration with strictly limited usage, so that distributed coordination is possible without state bloat.

#### Acceptance Criteria

1. WHERE etcd is enabled, THE Orchestrator SHALL store node existence information in etcd
2. WHERE etcd is enabled, THE Orchestrator SHALL store capability provider metadata in etcd
3. WHERE etcd is enabled, THE Orchestrator SHALL NOT store Wasm instance state in etcd
4. WHERE etcd is enabled, THE Orchestrator SHALL NOT store execution logs in etcd
5. WHERE etcd is enabled, THE Orchestrator SHALL NOT store desired state in etcd
6. THE Orchestrator SHALL operate without etcd in single-node mode

### Requirement 8: Resilient Crash Handling

**User Story:** As an operator, I want the system to handle instance crashes gracefully, so that individual failures don't compromise the entire system.

#### Acceptance Criteria

1. WHEN a Wasm instance crashes, THE Orchestrator SHALL continue operating normally
2. WHEN a node fails, THE Orchestrator SHALL remain operational on other nodes
3. THE Orchestrator SHALL treat instance crashes as acceptable events
4. WHEN an instance crashes, THE Orchestrator SHALL NOT lose system-level state
5. THE Orchestrator SHALL support fault tolerance through stateless design

### Requirement 9: Execution Facts Model

**User Story:** As a system architect, I want the orchestrator to manage execution facts rather than desired state, so that the system reflects reality rather than intent.

#### Acceptance Criteria

1. THE Orchestrator SHALL record events that have occurred during execution
2. THE Orchestrator SHALL NOT perform desired state reconciliation
3. THE Orchestrator SHALL NOT maintain a desired state specification for instances
4. WHEN queried, THE Orchestrator SHALL report actual execution status rather than intended status
5. THE Orchestrator SHALL focus on "what happened" rather than "what should be"

### Requirement 10: Phase 1 - Single Node Implementation

**User Story:** As a developer, I want to implement a single-node orchestrator first, so that core functionality can be validated before distribution.

#### Acceptance Criteria

1. THE Orchestrator SHALL operate on a single node without distributed coordination
2. THE Orchestrator SHALL manage Wasm instances using in-memory data structures
3. THE Orchestrator SHALL support at least one KV-type capability provider
4. THE Orchestrator SHALL NOT require etcd in Phase 1
5. THE Orchestrator SHALL provide a foundation for multi-node expansion

### Requirement 11: Phase 2 - Node Agent Separation and etcd

**User Story:** As a system architect, I want to separate the node agent and introduce optional etcd, so that the system can scale to multiple nodes.

#### Acceptance Criteria

1. THE Orchestrator SHALL separate Control Plane and Node Agent into distinct components
2. WHERE etcd is enabled, THE Orchestrator SHALL use etcd for metadata storage only
3. THE Node_Agent SHALL communicate with the Control Plane over a defined protocol
4. THE Orchestrator SHALL support multiple Node Agents running on different nodes
5. THE Orchestrator SHALL maintain backward compatibility with Phase 1 single-node operation

### Requirement 12: Phase 3 - Provider Distribution and micro-kvm

**User Story:** As a system architect, I want to distribute capability providers and support micro-kvm execution, so that the system can handle advanced deployment scenarios.

#### Acceptance Criteria

1. THE Orchestrator SHALL support capability providers running on different nodes than Wasm instances
2. THE Orchestrator SHALL support micro-kvm as an execution path for Wasm instances
3. THE Orchestrator SHALL route capability invocations across the network when providers are distributed
4. THE Orchestrator SHALL maintain the same security model with distributed providers
5. THE Orchestrator SHALL support both local and remote capability provider configurations

### Requirement 13: Control Plane API

**User Story:** As a client application, I want to interact with the orchestrator through a well-defined API, so that I can manage Wasm instances programmatically.

#### Acceptance Criteria

1. THE Control_Plane SHALL expose an API for starting Wasm instances
2. THE Control_Plane SHALL expose an API for stopping Wasm instances
3. THE Control_Plane SHALL expose an API for querying instance status
4. THE Control_Plane SHALL expose an API for managing capability assignments
5. WHEN an API request is received, THE Control_Plane SHALL validate the request before processing
6. WHEN an API operation fails, THE Control_Plane SHALL return descriptive error information

### Requirement 14: Instance Isolation

**User Story:** As a security engineer, I want Wasm instances to be isolated from each other, so that one instance cannot interfere with another.

#### Acceptance Criteria

1. THE Wasm_Runtime SHALL execute each Wasm instance in an isolated environment
2. THE Orchestrator SHALL prevent Wasm instances from accessing each other's memory
3. THE Orchestrator SHALL prevent Wasm instances from accessing each other's capability assignments
4. WHEN multiple instances run on the same node, THE Orchestrator SHALL maintain isolation boundaries
5. THE Orchestrator SHALL ensure capability provider access is scoped to the requesting instance

### Requirement 15: Restart Policy Configuration

**User Story:** As an operator, I want to configure restart policies for Wasm instances, so that I can control automatic recovery behavior.

#### Acceptance Criteria

1. THE Orchestrator SHALL support a "no restart" policy where crashed instances are not restarted
2. THE Orchestrator SHALL support an "always restart" policy where crashed instances are automatically restarted
3. THE Orchestrator SHALL support a "restart with backoff" policy with configurable delay
4. WHEN a restart policy is configured, THE Node_Agent SHALL enforce the policy on instance crashes
5. THE Orchestrator SHALL allow restart policy to be specified per instance

### Requirement 16: Capability Provider Lifecycle

**User Story:** As an operator, I want to manage capability provider lifecycle independently from Wasm instances, so that providers can be updated without affecting running instances.

#### Acceptance Criteria

1. THE Orchestrator SHALL support starting capability providers independently
2. THE Orchestrator SHALL support stopping capability providers independently
3. WHEN a capability provider is stopped, THE Orchestrator SHALL handle requests from instances gracefully
4. THE Orchestrator SHALL support updating capability provider implementations
5. THE Orchestrator SHALL maintain capability provider metadata separately from instance metadata
