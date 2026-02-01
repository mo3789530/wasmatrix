defmodule WasmatrixControl.Models do
  @moduledoc """
  Data models for the Wasmatrix control plane.

  This module provides the core data structures used throughout the system:
  - Node: Represents compute nodes in the cluster
  - WasmModule: WebAssembly modules with versioning
  - SchedulingDecision: Placement decisions
  - EventMessage: System events and messages
  """

  alias WasmatrixControl.Models.Node
  alias WasmatrixControl.Models.WasmModule
  alias WasmatrixControl.Models.SchedulingDecision
  alias WasmatrixControl.Models.EventMessage

  defdelegate node_new(attrs), to: Node, as: :new
  defdelegate node_validate(node), to: Node, as: :validate
  defdelegate node_heartbeat(node), to: Node, as: :heartbeat
  defdelegate node_healthy?(node), to: Node, as: :healthy?

  defdelegate module_new(attrs), to: WasmModule, as: :new
  defdelegate module_validate(module), to: WasmModule, as: :validate
  defdelegate module_new_version(module, attrs), to: WasmModule, as: :new_version
  defdelegate module_verify_signature?(module, key), to: WasmModule, as: :verify_signature?

  defdelegate decision_new(attrs), to: SchedulingDecision, as: :new
  defdelegate decision_validate(decision), to: SchedulingDecision, as: :validate
  defdelegate decision_accept(decision), to: SchedulingDecision, as: :accept
  defdelegate decision_reject(decision, reason), to: SchedulingDecision, as: :reject

  defdelegate event_new(attrs), to: EventMessage, as: :new
  defdelegate event_validate(event), to: EventMessage, as: :validate
  defdelegate event_reply(event, type, payload), to: EventMessage, as: :reply
end
