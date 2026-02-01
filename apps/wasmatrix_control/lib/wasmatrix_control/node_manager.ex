defmodule WasmatrixControl.NodeManager do
  @moduledoc """
  GenServer for managing node registration and health monitoring.

  Handles:
  - Node registration with capability tracking
  - Periodic health checks with heartbeat monitoring
  - Real-time node inventory with status updates
  - Node failure detection and recovery

  Requirements: 1.1, 1.2, 1.3, 1.4, 1.5
  """

  use GenServer

  alias WasmatrixControl.Models.Node
  alias WasmatrixControl.Models.EventMessage

  @type state :: %{
          nodes: %{String.t() => Node.t()},
          health_checks: %{String.t() => reference()},
          heartbeat_interval: non_neg_integer(),
          node_timeout: non_neg_integer()
        }

  # Client API

  def start_link(opts \\ []) do
    name = opts[:name] || __MODULE__
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  @doc """
  Registers a new node in the cluster.
  """
  def register_node(server \\ __MODULE__, node_attrs) do
    GenServer.call(server, {:register, node_attrs})
  end

  @doc """
  Updates node heartbeat timestamp.
  """
  def heartbeat(server \\ __MODULE__, node_id) do
    GenServer.cast(server, {:heartbeat, node_id})
  end

  @doc """
  Gets a node by ID.
  """
  def get_node(server \\ __MODULE__, node_id) do
    GenServer.call(server, {:get_node, node_id})
  end

  @doc """
  Lists all registered nodes.
  """
  def list_nodes(server \\ __MODULE__) do
    GenServer.call(server, :list_nodes)
  end

  @doc """
  Lists nodes filtered by status.
  """
  def list_nodes_by_status(server \\ __MODULE__, status) do
    GenServer.call(server, {:list_nodes_by_status, status})
  end

  @doc """
  Updates node metadata.
  """
  def update_node_metadata(server \\ __MODULE__, node_id, metadata) do
    GenServer.call(server, {:update_metadata, node_id, metadata})
  end

  @doc """
  Unregisters a node from the cluster.
  """
  def unregister_node(server \\ __MODULE__, node_id) do
    GenServer.call(server, {:unregister, node_id})
  end

  @doc """
  Gets healthy nodes that have all required capabilities.
  """
  def find_capable_nodes(server \\ __MODULE__, capabilities) do
    GenServer.call(server, {:find_capable, capabilities})
  end

  # Server Callbacks

  @impl true
  def init(opts) do
    heartbeat_interval =
      opts[:heartbeat_interval] ||
        Application.get_env(:wasmatrix_control, :node_heartbeat_interval, 5_000)

    node_timeout =
      opts[:node_timeout] ||
        Application.get_env(:wasmatrix_control, :node_timeout, 15_000)

    state = %{
      nodes: %{},
      health_checks: %{},
      heartbeat_interval: heartbeat_interval,
      node_timeout: node_timeout
    }

    # Schedule periodic health check
    schedule_health_check(heartbeat_interval)

    {:ok, state}
  end

  @impl true
  def handle_call({:register, node_attrs}, _from, state) do
    case Node.new(node_attrs) do
      {:ok, node} ->
        # Start health check timer for this node
        timer = Process.send_after(self(), {:check_health, node.id}, state.heartbeat_interval)

        new_state =
          state
          |> put_in([:nodes, node.id], node)
          |> put_in([:health_checks, node.id], timer)

        # Publish node registered event
        {:ok, event} = EventMessage.node_registered(node.id, %{capabilities: node.capabilities})
        publish_event(event)

        {:reply, {:ok, node}, new_state}

      {:error, reason} ->
        {:reply, {:error, reason}, state}
    end
  end

  @impl true
  def handle_call({:get_node, node_id}, _from, state) do
    reply =
      case Map.get(state.nodes, node_id) do
        nil -> {:error, :not_found}
        node -> {:ok, node}
      end

    {:reply, reply, state}
  end

  @impl true
  def handle_call(:list_nodes, _from, state) do
    nodes = Map.values(state.nodes)
    {:reply, {:ok, nodes}, state}
  end

  @impl true
  def handle_call({:list_nodes_by_status, status}, _from, state) do
    nodes =
      state.nodes
      |> Map.values()
      |> Enum.filter(&(&1.status == status))

    {:reply, {:ok, nodes}, state}
  end

  @impl true
  def handle_call({:update_metadata, node_id, metadata}, _from, state) do
    case Map.get(state.nodes, node_id) do
      nil ->
        {:reply, {:error, :not_found}, state}

      node ->
        updated_node = %{
          node
          | metadata: Map.merge(node.metadata, metadata),
            updated_at: DateTime.utc_now()
        }

        new_state = put_in(state, [:nodes, node_id], updated_node)
        {:reply, {:ok, updated_node}, new_state}
    end
  end

  @impl true
  def handle_call({:unregister, node_id}, _from, state) do
    case Map.get(state.nodes, node_id) do
      nil ->
        {:reply, {:error, :not_found}, state}

      _node ->
        # Cancel health check timer
        if timer = state.health_checks[node_id] do
          Process.cancel_timer(timer)
        end

        new_state =
          state
          |> update_in([:nodes], &Map.delete(&1, node_id))
          |> update_in([:health_checks], &Map.delete(&1, node_id))

        # Publish node offline event
        {:ok, event} =
          EventMessage.new(
            type: "node.offline",
            source: node_id,
            payload: %{reason: :unregistered}
          )

        publish_event(event)

        {:reply, :ok, new_state}
    end
  end

  @impl true
  def handle_call({:find_capable, capabilities}, _from, state) do
    nodes =
      state.nodes
      |> Map.values()
      |> Enum.filter(&Node.healthy?/1)
      |> Enum.filter(&Node.has_capabilities?(&1, capabilities))

    {:reply, {:ok, nodes}, state}
  end

  @impl true
  def handle_cast({:heartbeat, node_id}, state) do
    case Map.get(state.nodes, node_id) do
      nil ->
        {:noreply, state}

      node ->
        updated_node = Node.heartbeat(node)
        new_state = put_in(state, [:nodes, node_id], updated_node)

        # Reschedule health check
        if timer = state.health_checks[node_id] do
          Process.cancel_timer(timer)
        end

        timer = Process.send_after(self(), {:check_health, node_id}, state.heartbeat_interval)
        new_state = put_in(new_state, [:health_checks, node_id], timer)

        {:noreply, new_state}
    end
  end

  @impl true
  def handle_info({:check_health, node_id}, state) do
    case Map.get(state.nodes, node_id) do
      nil ->
        {:noreply, state}

      node ->
        new_state =
          if Node.healthy?(node) do
            # Node is healthy, reschedule check
            timer = Process.send_after(self(), {:check_health, node_id}, state.heartbeat_interval)
            put_in(state, [:health_checks, node_id], timer)
          else
            # Node failed health check, mark as offline
            updated_node = %{node | status: :offline}

            # Publish node offline event
            {:ok, event} =
              EventMessage.new(
                type: "node.offline",
                source: node_id,
                payload: %{reason: :heartbeat_timeout}
              )

            publish_event(event)

            put_in(state, [:nodes, node_id], updated_node)
          end

        {:noreply, new_state}
    end
  end

  @impl true
  def handle_info(:periodic_health_check, state) do
    # Check all nodes
    Enum.each(state.nodes, fn {node_id, _node} ->
      send(self(), {:check_health, node_id})
    end)

    schedule_health_check(state.heartbeat_interval)
    {:noreply, state}
  end

  # Private Functions

  defp schedule_health_check(interval) do
    Process.send_after(self(), :periodic_health_check, interval)
  end

  defp publish_event(%EventMessage{} = _event) do
    # In production, this would publish to EventSystem (Task 7)
    # For now, just a placeholder
    :ok
  end
end
