defmodule WasmatrixControl.Models.EventMessage do
  @moduledoc """
  Represents an event in the Wasmatrix system.

  Events are used for triggering module execution, notifying state changes,
  and communicating between components.
  """

  @type t :: %__MODULE__{
          id: String.t(),
          type: String.t(),
          source: String.t(),
          target: String.t() | nil,
          payload: map(),
          metadata: map(),
          timestamp: DateTime.t(),
          priority: :low | :normal | :high | :critical,
          ttl: non_neg_integer(),
          correlation_id: String.t() | nil
        }

  @enforce_keys [:type, :source]
  defstruct [
    :id,
    :type,
    :source,
    :target,
    payload: %{},
    metadata: %{},
    timestamp: nil,
    priority: :normal,
    ttl: 60,
    correlation_id: nil
  ]

  # Common event types
  @event_types [
    # System events
    "node.registered",
    "node.heartbeat",
    "node.offline",
    "module.uploaded",
    "module.deployed",
    "module.rollback",
    "module.executed",
    "module.failed",
    "scheduling.request",
    "scheduling.decision",
    "state.changed",
    "system.alert",
    # Test events (for testing)
    "test.event",
    "flood.event",
    "stat.event",
    "request",
    "response",
    # User/Domain events
    "user.created",
    "user.updated",
    "user.deleted",
    "user.login"
  ]

  @doc """
  Creates a new event message.
  """
  def new(attrs) when is_map(attrs) or is_list(attrs) do
    attrs = Map.new(attrs)

    event = %__MODULE__{
      id: attrs[:id] || generate_id(),
      type: attrs[:type],
      source: attrs[:source],
      target: attrs[:target],
      payload: attrs[:payload] || %{},
      metadata: attrs[:metadata] || %{},
      timestamp: DateTime.utc_now(),
      priority: attrs[:priority] || :normal,
      ttl: attrs[:ttl] || 60,
      correlation_id: attrs[:correlation_id] || generate_id()
    }

    case validate(event) do
      {:ok, valid_event} -> {:ok, valid_event}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc """
  Validates an event message.
  """
  def validate(%__MODULE__{} = event) do
    errors = []

    errors =
      if is_nil(event.type) or event.type == "", do: ["type is required" | errors], else: errors

    errors =
      if is_nil(event.source) or event.source == "",
        do: ["source is required" | errors],
        else: errors

    errors = unless event.type in @event_types, do: ["invalid event type" | errors], else: errors

    errors =
      unless event.priority in [:low, :normal, :high, :critical],
        do: ["invalid priority" | errors],
        else: errors

    errors = if event.ttl < 0, do: ["ttl must be non-negative" | errors], else: errors

    if errors == [] do
      {:ok, event}
    else
      {:error, Enum.reverse(errors)}
    end
  end

  @doc """
  Checks if the event has expired based on TTL.
  """
  def expired?(%__MODULE__{timestamp: timestamp, ttl: ttl}) do
    expiry = DateTime.add(timestamp, ttl, :second)
    DateTime.compare(DateTime.utc_now(), expiry) == :gt
  end

  @doc """
  Returns the remaining time in seconds before expiration.
  """
  def remaining_ttl(%__MODULE__{timestamp: timestamp, ttl: ttl}) do
    expiry = DateTime.add(timestamp, ttl, :second)
    now = DateTime.utc_now()

    case DateTime.compare(expiry, now) do
      :gt -> DateTime.diff(expiry, now, :second)
      _ -> 0
    end
  end

  @doc """
  Creates a reply event with the same correlation ID.
  """
  def reply(%__MODULE__{} = original, reply_type, payload \\ %{}) do
    new(
      type: reply_type,
      source: "system",
      target: original.source,
      payload: payload,
      correlation_id: original.correlation_id,
      priority: original.priority
    )
  end

  @doc """
  Adds metadata to the event.
  """
  def put_metadata(%__MODULE__{metadata: meta} = event, key, value) do
    %{event | metadata: Map.put(meta, key, value)}
  end

  @doc """
  Returns all valid event types.
  """
  def event_types, do: @event_types

  # Predefined event builders

  def node_registered(node_id, metadata \\ %{}) do
    new(
      type: "node.registered",
      source: node_id,
      payload: %{node_id: node_id},
      metadata: metadata
    )
  end

  def node_heartbeat(node_id, status \\ %{}) do
    new(
      type: "node.heartbeat",
      source: node_id,
      payload: %{node_id: node_id, status: status},
      priority: :low
    )
  end

  def module_deployed(module_id, node_id, metadata \\ %{}) do
    new(
      type: "module.deployed",
      source: "scheduler",
      target: node_id,
      payload: %{module_id: module_id, node_id: node_id},
      metadata: metadata
    )
  end

  def scheduling_request(module_id, constraints \\ %{}) do
    new(
      type: "scheduling.request",
      source: "api",
      payload: %{module_id: module_id, constraints: constraints},
      priority: :high
    )
  end

  defp generate_id do
    bytes = :crypto.strong_rand_bytes(8)
    "evt-" <> Base.encode16(bytes, case: :lower)
  end
end

defimpl Jason.Encoder, for: WasmatrixControl.Models.EventMessage do
  def encode(%WasmatrixControl.Models.EventMessage{} = event, opts) do
    event
    |> Map.from_struct()
    |> Enum.map(fn
      {k, %DateTime{} = dt} -> {k, DateTime.to_iso8601(dt)}
      {k, v} -> {k, v}
    end)
    |> Enum.into(%{})
    |> Jason.Encode.map(opts)
  end
end
