defmodule WasmatrixControl.Models.Node do
  @moduledoc """
  Represents a compute node in the Wasmatrix cluster.

  A node has a unique identifier, network address, capabilities,
  current status, and metadata for scheduling decisions.
  """

  @type t :: %__MODULE__{
          id: String.t(),
          hostname: String.t(),
          address: String.t(),
          port: non_neg_integer(),
          status: :online | :offline | :maintenance | :degraded,
          capabilities: [String.t()],
          resources: map(),
          metadata: map(),
          last_heartbeat: DateTime.t() | nil,
          registered_at: DateTime.t(),
          fault_domain: String.t(),
          architecture: String.t(),
          version: String.t()
        }

  @enforce_keys [:id, :hostname, :address]
  defstruct [
    :id,
    :hostname,
    :address,
    port: 50051,
    status: :offline,
    capabilities: [],
    resources: %{},
    metadata: %{},
    last_heartbeat: nil,
    registered_at: nil,
    updated_at: nil,
    fault_domain: "default",
    architecture: "x86_64",
    version: "0.1.0"
  ]

  @doc """
  Creates a new node with the required fields.
  """
  def new(attrs) when is_map(attrs) or is_list(attrs) do
    attrs = Map.new(attrs)

    node = %__MODULE__{
      id: attrs[:id] || generate_id(),
      hostname: attrs[:hostname],
      address: attrs[:address],
      port: attrs[:port] || 50051,
      status: attrs[:status] || :offline,
      capabilities: attrs[:capabilities] || [],
      resources: attrs[:resources] || %{},
      metadata: attrs[:metadata] || %{},
      registered_at: DateTime.utc_now(),
      fault_domain: attrs[:fault_domain] || "default",
      architecture: attrs[:architecture] || "x86_64",
      version: attrs[:version] || "0.1.0"
    }

    case validate(node) do
      {:ok, valid_node} -> {:ok, valid_node}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc """
  Validates a node struct.
  """
  def validate(%__MODULE__{} = node) do
    errors = []

    errors = if is_nil(node.id) or node.id == "", do: ["id is required" | errors], else: errors

    errors =
      if is_nil(node.hostname) or node.hostname == "",
        do: ["hostname is required" | errors],
        else: errors

    errors =
      if is_nil(node.address) or node.address == "",
        do: ["address is required" | errors],
        else: errors

    errors =
      if node.port < 1 or node.port > 65535,
        do: ["port must be between 1 and 65535" | errors],
        else: errors

    errors =
      unless node.status in [:online, :offline, :maintenance, :degraded],
        do: ["invalid status" | errors],
        else: errors

    if errors == [] do
      {:ok, node}
    else
      {:error, Enum.reverse(errors)}
    end
  end

  @doc """
  Updates node status and heartbeat timestamp.
  """
  def heartbeat(%__MODULE__{} = node) do
    %{node | status: :online, last_heartbeat: DateTime.utc_now()}
  end

  @doc """
  Checks if the node is healthy based on last heartbeat.
  """
  def healthy?(%__MODULE__{last_heartbeat: nil}), do: false

  def healthy?(%__MODULE__{last_heartbeat: heartbeat, status: status}) do
    if status == :online do
      timeout_ms = Application.get_env(:wasmatrix_control, :node_timeout, 15_000)
      last_heartbeat_ms = DateTime.to_unix(heartbeat, :millisecond)
      now_ms = DateTime.to_unix(DateTime.utc_now(), :millisecond)

      now_ms - last_heartbeat_ms < timeout_ms
    else
      false
    end
  end

  @doc """
  Returns true if the node has all the specified capabilities.
  """
  def has_capabilities?(%__MODULE__{capabilities: caps}, required) do
    required_list = List.wrap(required)
    Enum.all?(required_list, &(&1 in caps))
  end

  defp generate_id do
    bytes = :crypto.strong_rand_bytes(8)
    "node-" <> Base.encode16(bytes, case: :lower)
  end
end

defimpl Jason.Encoder, for: WasmatrixControl.Models.Node do
  def encode(%WasmatrixControl.Models.Node{} = node, opts) do
    node
    |> Map.from_struct()
    |> Enum.map(fn
      {k, %DateTime{} = dt} -> {k, DateTime.to_iso8601(dt)}
      {k, v} -> {k, v}
    end)
    |> Enum.into(%{})
    |> Jason.Encode.map(opts)
  end
end
