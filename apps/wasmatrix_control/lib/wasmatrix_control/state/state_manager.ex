defmodule WasmatrixControl.State.StateManager do
  @moduledoc """
  GenServer for distributed state management with CRDT-based consistency.

  Provides:
  - CRDT-based distributed state management (DeltaCrdt)
  - External KV store and stream integrations
  - Local caching similar to BEAM's ETS
  - State consistency validation and conflict resolution
  - Seamless API integration with Wasm module capabilities

  Requirements: 10.1, 10.2, 10.3, 10.4, 10.5
  """

  use GenServer

  alias WasmatrixControl.Models.EventMessage

  @type state_value :: term()
  @type crdt_state :: DeltaCrdt.t()

  @type state_entry :: %{
          key: String.t(),
          value: state_value(),
          version: non_neg_integer(),
          timestamp: DateTime.t(),
          node_id: String.t(),
          ttl: non_neg_integer() | nil
        }

  @type state :: %{
          crdt: crdt_state(),
          cache: :ets.tab(),
          backend: module() | nil,
          backend_state: term(),
          config: %{
            sync_interval: non_neg_integer(),
            max_cache_size: non_neg_integer(),
            default_ttl: non_neg_integer() | nil,
            conflict_resolution: :last_write_wins | :vector_clock
          },
          stats: %{
            reads: non_neg_integer(),
            writes: non_neg_integer(),
            cache_hits: non_neg_integer(),
            cache_misses: non_neg_integer(),
            conflicts_resolved: non_neg_integer()
          }
        }

  @default_config %{
    sync_interval: 5_000,
    max_cache_size: 10_000,
    default_ttl: nil,
    conflict_resolution: :last_write_wins
  }

  # Client API

  def start_link(opts \\ []) do
    name = opts[:name] || __MODULE__
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  @doc """
  Reads a value from the state store.
  Checks local cache first, then CRDT, then backend if configured.
  """
  def get(server \\ __MODULE__, key, opts \\ []) do
    GenServer.call(server, {:get, key, opts}, 5_000)
  end

  @doc """
  Writes a value to the state store.
  Updates CRDT, local cache, and backend if configured.
  """
  def put(server \\ __MODULE__, key, value, opts \\ []) do
    GenServer.call(server, {:put, key, value, opts}, 5_000)
  end

  @doc """
  Deletes a key from the state store.
  """
  def delete(server \\ __MODULE__, key) do
    GenServer.call(server, {:delete, key}, 5_000)
  end

  @doc """
  Performs a compare-and-swap operation for atomic updates.
  """
  def cas(server \\ __MODULE__, key, expected_value, new_value) do
    GenServer.call(server, {:cas, key, expected_value, new_value}, 5_000)
  end

  @doc """
  Lists all keys in the state store with optional prefix filter.
  """
  def keys(server \\ __MODULE__, prefix \\ nil) do
    GenServer.call(server, {:keys, prefix}, 5_000)
  end

  @doc """
  Gets state statistics.
  """
  def get_stats(server \\ __MODULE__) do
    GenServer.call(server, :get_stats)
  end

  @doc """
  Clears the local cache (not the CRDT or backend).
  """
  def clear_cache(server \\ __MODULE__) do
    GenServer.call(server, :clear_cache)
  end

  @doc """
  Syncs state with other nodes in the cluster.
  """
  def sync(server \\ __MODULE__) do
    GenServer.cast(server, :sync)
  end

  @doc """
  Subscribes to changes for a specific key or pattern.
  """
  def subscribe_changes(server \\ __MODULE__, pattern) do
    GenServer.call(server, {:subscribe_changes, pattern, self()}, 5_000)
  end

  # Server Callbacks

  @impl true
  def init(opts) do
    node_id = opts[:node_id] || generate_node_id()
    config = Map.merge(@default_config, opts[:config] || %{})

    # Initialize CRDT with start_link
    {:ok, crdt} = DeltaCrdt.start_link(DeltaCrdt.AWLWWMap, sync_interval: config.sync_interval)

    # Initialize local ETS cache
    cache =
      :ets.new(:state_cache, [
        :set,
        :protected,
        read_concurrency: true,
        write_concurrency: true
      ])

    # Initialize backend if configured
    backend = opts[:backend]

    backend_state =
      if backend do
        {:ok, state} = backend.init(opts[:backend_opts] || [])
        state
      else
        nil
      end

    state = %{
      crdt: crdt,
      cache: cache,
      backend: backend,
      backend_state: backend_state,
      config: config,
      node_id: node_id,
      change_subscribers: %{},
      stats: %{
        reads: 0,
        writes: 0,
        cache_hits: 0,
        cache_misses: 0,
        conflicts_resolved: 0
      }
    }

    # Schedule periodic cache cleanup
    schedule_cache_cleanup(config.default_ttl)

    {:ok, state}
  end

  @impl true
  def handle_call({:get, key, _opts}, _from, state) do
    # First check cache
    cache_result = lookup_cache(state.cache, key)

    {value, state} =
      case cache_result do
        {:hit, cached_value} ->
          {cached_value, update_in(state, [:stats, :cache_hits], &(&1 + 1))}

        :miss ->
          # Check CRDT
          crdt_value = DeltaCrdt.get(state.crdt, key)

          # Unwrap entry if it's a wrapped entry struct
          raw_value =
            if is_map(crdt_value) and Map.has_key?(crdt_value, :value) do
              crdt_value.value
            else
              crdt_value
            end

          # If found in CRDT, update cache
          if crdt_value != nil do
            insert_cache(state.cache, key, raw_value, state.config.default_ttl)
          end

          {raw_value, update_in(state, [:stats, :cache_misses], &(&1 + 1))}
      end

    # Check backend if not found and backend configured
    final_value =
      if value == nil and state.backend != nil do
        case state.backend.get(key, state.backend_state) do
          {:ok, backend_value, new_backend_state} ->
            # Update CRDT and cache with backend value
            entry = wrap_entry(key, backend_value, state)
            DeltaCrdt.put(state.crdt, key, entry)
            insert_cache(state.cache, key, backend_value, state.config.default_ttl)
            backend_value

          {:error, _} ->
            nil
        end
      else
        value
      end

    state = update_in(state, [:stats, :reads], &(&1 + 1))

    {:reply, {:ok, final_value}, state}
  end

  @impl true
  def handle_call({:put, key, value, opts}, _from, state) do
    ttl = opts[:ttl] || state.config.default_ttl
    entry = wrap_entry(key, value, state, ttl)

    # Update CRDT
    DeltaCrdt.put(state.crdt, key, entry)

    # Update cache
    insert_cache(state.cache, key, value, ttl)

    # Update backend if configured
    state =
      if state.backend do
        case state.backend.put(key, value, state.backend_state) do
          {:ok, new_backend_state} ->
            %{state | backend_state: new_backend_state}

          {:error, _} ->
            state
        end
      else
        state
      end

    # Notify subscribers
    notify_change_subscribers(state, key, value)

    state = update_in(state, [:stats, :writes], &(&1 + 1))

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:delete, key}, _from, state) do
    # Remove from CRDT
    DeltaCrdt.delete(state.crdt, key)

    # Remove from cache
    :ets.delete(state.cache, key)

    # Remove from backend if configured
    state =
      if state.backend do
        case state.backend.delete(key, state.backend_state) do
          {:ok, new_backend_state} ->
            %{state | backend_state: new_backend_state}

          {:error, _} ->
            state
        end
      else
        state
      end

    # Notify subscribers
    notify_change_subscribers(state, key, nil)

    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:cas, key, expected_value, new_value}, from, state) do
    # Read current value
    current =
      case lookup_cache(state.cache, key) do
        {:hit, value} -> value
        :miss -> DeltaCrdt.get(state.crdt, key)
      end

    # Check if matches expected
    if current == expected_value do
      # Perform update
      handle_call({:put, key, new_value, []}, from, state)
    else
      {:reply, {:error, :cas_failed}, state}
    end
  end

  @impl true
  def handle_call({:keys, prefix}, _from, state) do
    # Get all keys from CRDT
    all_keys =
      DeltaCrdt.to_map(state.crdt)
      |> Enum.map(fn {key, _value} -> key end)

    # Filter by prefix if provided
    filtered =
      if prefix do
        Enum.filter(all_keys, &String.starts_with?(&1, prefix))
      else
        all_keys
      end

    {:reply, {:ok, filtered}, state}
  end

  @impl true
  def handle_call(:get_stats, _from, state) do
    # Add cache size to stats
    cache_size = :ets.info(state.cache, :size)
    stats = Map.put(state.stats, :cache_size, cache_size)
    stats = Map.put(stats, :crdt_size, DeltaCrdt.to_map(state.crdt) |> map_size())

    {:reply, {:ok, stats}, state}
  end

  @impl true
  def handle_call(:clear_cache, _from, state) do
    :ets.delete_all_objects(state.cache)
    {:reply, :ok, state}
  end

  @impl true
  def handle_call({:subscribe_changes, pattern, pid}, _from, state) do
    ref = Process.monitor(pid)
    subscribers = Map.put(state.change_subscribers, ref, %{pattern: pattern, pid: pid})
    {:reply, {:ok, ref}, %{state | change_subscribers: subscribers}}
  end

  @impl true
  def handle_cast(:sync, state) do
    # CRDT syncs automatically, but we can force a read to ensure consistency
    _ = DeltaCrdt.to_map(state.crdt)
    {:noreply, state}
  end

  @impl true
  def handle_info({:DOWN, ref, :process, _pid, _reason}, state) do
    # Clean up subscriber
    subscribers = Map.delete(state.change_subscribers, ref)
    {:noreply, %{state | change_subscribers: subscribers}}
  end

  @impl true
  def handle_info(:cleanup_cache, state) do
    # Remove expired entries from cache
    now = System.monotonic_time(:second)

    # ETS doesn't support easy TTL cleanup, so we'll just clear old entries
    # In production, you'd want a more sophisticated cleanup strategy
    if :ets.info(state.cache, :size) > state.config.max_cache_size do
      :ets.delete_all_objects(state.cache)
    end

    # Schedule next cleanup
    schedule_cache_cleanup(state.config.default_ttl)

    {:noreply, state}
  end

  # Private Functions

  defp generate_node_id do
    ("state-node-" <> :crypto.strong_rand_bytes(4)) |> Base.encode16(case: :lower)
  end

  defp wrap_entry(key, value, state, ttl \\ nil) do
    %{
      key: key,
      value: value,
      version: generate_version(),
      timestamp: DateTime.utc_now(),
      node_id: state.node_id,
      ttl: ttl
    }
  end

  defp generate_version do
    System.unique_integer([:positive])
  end

  defp lookup_cache(cache, key) do
    case :ets.lookup(cache, key) do
      [{^key, value, _expires}] -> {:hit, value}
      [] -> :miss
    end
  end

  defp insert_cache(cache, key, value, ttl) do
    expires =
      if ttl do
        System.monotonic_time(:second) + ttl
      else
        :infinity
      end

    :ets.insert(cache, {key, value, expires})
  end

  defp schedule_cache_cleanup(_ttl) do
    Process.send_after(self(), :cleanup_cache, 60_000)
  end

  defp notify_change_subscribers(state, key, value) do
    Enum.each(state.change_subscribers, fn {_ref, %{pattern: pattern, pid: pid}} ->
      if matches_pattern?(key, pattern) do
        send(pid, {:state_change, key, value})
      end
    end)
  end

  defp matches_pattern?(key, pattern) when is_binary(pattern) do
    String.starts_with?(key, pattern)
  end

  defp matches_pattern?(key, %Regex{} = pattern) do
    Regex.match?(pattern, key)
  end

  defp matches_pattern?(_key, _pattern), do: false
end
