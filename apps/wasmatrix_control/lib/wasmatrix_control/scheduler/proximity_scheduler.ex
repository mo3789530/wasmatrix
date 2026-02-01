defmodule WasmatrixControl.Scheduler.ProximityScheduler do
  @moduledoc """
  GenServer for proximity-based scheduling decisions.

  Implements multi-criteria decision making for WebAssembly module placement:
  - Proximity-based node prioritization (latency, geographic location)
  - Data locality awareness
  - Fault domain distribution for high availability
  - Architecture compatibility matching
  - Resource availability scoring

  Performance target: 5ms placement decisions for edge workloads

  Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 11.2, 11.5
  """

  use GenServer

  alias WasmatrixControl.Models.Node
  alias WasmatrixControl.Models.WasmModule
  alias WasmatrixControl.Models.SchedulingDecision
  alias WasmatrixControl.Models.EventMessage
  alias WasmatrixControl.NodeManager

  @type scoring_factors :: %{
          proximity: float(),
          resources: float(),
          fault_domain: float(),
          architecture: float(),
          capabilities: float()
        }

  @type constraints :: %{
          required_capabilities: [String.t()],
          target_fault_domains: [String.t()],
          excluded_nodes: [String.t()],
          preferred_locality: String.t() | nil,
          min_resources: map()
        }

  @default_constraints %{
    required_capabilities: [],
    target_fault_domains: [],
    excluded_nodes: [],
    preferred_locality: nil,
    min_resources: %{}
  }

  @default_weights %{
    proximity: 0.30,
    resources: 0.25,
    fault_domain: 0.20,
    architecture: 0.15,
    capabilities: 0.10
  }

  # Client API

  def start_link(opts \\ []) do
    name = opts[:name] || __MODULE__
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  @doc """
  Schedules a module execution by selecting the optimal node.

  Returns {:ok, SchedulingDecision} on success, {:error, reason} on failure.
  """
  def schedule(server \\ __MODULE__, module_id, constraints \\ %{}, opts \\ []) do
    GenServer.call(server, {:schedule, module_id, constraints, opts}, 10_000)
  end

  @doc """
  Schedules multiple modules with batch optimization.
  """
  def schedule_batch(server \\ __MODULE__, module_ids, constraints \\ %{}) do
    GenServer.call(server, {:schedule_batch, module_ids, constraints}, 30_000)
  end

  @doc """
  Pre-computes scheduling cache for common scenarios.
  """
  def precompute_cache(server \\ __MODULE__, scenarios) do
    GenServer.cast(server, {:precompute, scenarios})
  end

  @doc """
  Gets scheduling statistics.
  """
  def get_stats(server \\ __MODULE__) do
    GenServer.call(server, :get_stats)
  end

  @doc """
  Updates node scoring weights.
  """
  def update_weights(server \\ __MODULE__, weights) do
    GenServer.call(server, {:update_weights, weights})
  end

  # Server Callbacks

  @impl true
  def init(opts) do
    node_manager = opts[:node_manager] || NodeManager
    module_manager = opts[:module_manager] || WasmatrixControl.ModuleManager

    state = %{
      node_manager: node_manager,
      module_manager: module_manager,
      weights: Map.merge(@default_weights, opts[:weights] || %{}),
      cache: %{},
      stats: %{
        total_scheduled: 0,
        avg_decision_time_ms: 0,
        cache_hits: 0,
        cache_misses: 0
      }
    }

    {:ok, state}
  end

  @impl true
  def handle_call({:schedule, module_id, constraints, opts}, _from, state) do
    start_time = System.monotonic_time(:microsecond)

    # Check cache first if enabled
    cache_key = {module_id, constraints}

    {result, state} =
      case Map.get(state.cache, cache_key) do
        nil ->
          # Cache miss - perform full scheduling
          state = update_in(state, [:stats, :cache_misses], &(&1 + 1))
          {perform_scheduling(module_id, constraints, opts, state), state}

        cached_decision ->
          # Cache hit - verify decision is still valid
          if SchedulingDecision.expired?(cached_decision) do
            state = update_in(state, [:stats, :cache_misses], &(&1 + 1))
            new_state = %{state | cache: Map.delete(state.cache, cache_key)}
            {perform_scheduling(module_id, constraints, opts, new_state), new_state}
          else
            state = update_in(state, [:stats, :cache_hits], &(&1 + 1))
            {{:ok, cached_decision}, state}
          end
      end

    # Calculate decision time
    decision_time_us = System.monotonic_time(:microsecond) - start_time
    decision_time_ms = decision_time_us / 1000

    # Update stats
    state = update_stats(state, decision_time_ms)

    # Log performance warning if over target
    if decision_time_ms > 5.0 do
      IO.puts("⚠️  Scheduling decision took #{Float.round(decision_time_ms, 2)}ms (target: 5ms)")
    end

    # Cache successful decisions
    state =
      case result do
        {:ok, decision} ->
          # Default 60 second cache
          ttl = opts[:cache_ttl] || 60_000
          valid_until = DateTime.add(DateTime.utc_now(), trunc(ttl), :millisecond)
          decision = %{decision | valid_until: valid_until}
          %{state | cache: Map.put(state.cache, cache_key, decision)}

        _ ->
          state
      end

    {:reply, result, state}
  end

  @impl true
  def handle_call({:schedule_batch, module_ids, constraints}, _from, state) do
    start_time = System.monotonic_time(:microsecond)

    # Merge with defaults like single scheduling
    constraints = Map.merge(@default_constraints, constraints)

    # Get module requirements
    modules_result =
      Enum.reduce(module_ids, {:ok, []}, fn id, acc ->
        case acc do
          {:ok, modules} ->
            case get_module(state.module_manager, id) do
              {:ok, module} -> {:ok, [module | modules]}
              error -> error
            end

          error ->
            error
        end
      end)

    result =
      case modules_result do
        {:ok, modules} ->
          # Get all capable nodes
          all_capabilities =
            modules
            |> Enum.flat_map(& &1.capabilities)
            |> Enum.uniq()

          case NodeManager.find_capable_nodes(state.node_manager, all_capabilities) do
            {:ok, []} ->
              {:error, :no_capable_nodes}

            {:ok, capable_nodes} ->
              # Score all node-module combinations
              scored =
                for node <- capable_nodes, module <- modules do
                  score = calculate_score(node, module, constraints, state.weights)
                  {score, node, module}
                end

              # Sort by score descending and create decisions
              decisions =
                scored
                |> Enum.sort_by(fn {score, _, _} -> score end, :desc)
                |> Enum.take(length(modules))
                |> Enum.map(fn {score, node, module} ->
                  {:ok, decision} =
                    SchedulingDecision.new(
                      module_id: module.id,
                      node_id: node.id,
                      score: score,
                      priority: calculate_priority(module),
                      reason: format_reason(score, node, module),
                      factors: calculate_factors(node, module, constraints, state.weights)
                    )

                  decision
                end)

              {:ok, decisions}

            error ->
              error
          end

        error ->
          error
      end

    # Calculate batch time
    _batch_time_ms = (System.monotonic_time(:microsecond) - start_time) / 1000

    state = update_in(state, [:stats, :total_scheduled], &(&1 + length(module_ids)))

    {:reply, result, state}
  end

  @impl true
  def handle_call(:get_stats, _from, state) do
    {:reply, {:ok, state.stats}, state}
  end

  @impl true
  def handle_call({:update_weights, weights}, _from, state) do
    new_weights = Map.merge(state.weights, weights)

    # Validate weights sum to 1.0
    total = Enum.sum(Map.values(new_weights))

    normalized_weights =
      if abs(total - 1.0) > 0.01 do
        # Normalize to sum to 1.0
        Map.new(new_weights, fn {k, v} -> {k, v / total} end)
      else
        new_weights
      end

    # Clear cache since weights changed
    new_state = %{state | weights: normalized_weights, cache: %{}}

    {:reply, {:ok, normalized_weights}, new_state}
  end

  @impl true
  def handle_cast({:precompute, scenarios}, state) do
    # Pre-compute scheduling decisions for common scenarios
    new_cache =
      Enum.reduce(scenarios, state.cache, fn scenario, cache_acc ->
        case perform_scheduling(scenario.module_id, scenario.constraints, %{}, state) do
          {:ok, decision} ->
            cache_key = {scenario.module_id, scenario.constraints}
            Map.put(cache_acc, cache_key, decision)

          _ ->
            cache_acc
        end
      end)

    {:noreply, %{state | cache: new_cache}}
  end

  # Private Functions

  defp perform_scheduling(module_id, constraints, opts, state) do
    constraints = Map.merge(@default_constraints, constraints)

    # Get module
    case get_module(state.module_manager, module_id) do
      {:error, _} = error ->
        error

      {:ok, module} ->
        # Get all nodes and filter candidates
        case NodeManager.list_nodes(state.node_manager) do
          {:error, _} = error ->
            error

          {:ok, nodes} ->
            candidates = filter_candidates(nodes, module, constraints, state.node_manager)

            if candidates == [] do
              {:error, :no_suitable_nodes}
            else
              # Score all candidates
              scored_candidates =
                Enum.map(candidates, fn node ->
                  score = calculate_score(node, module, constraints, state.weights)
                  {score, node}
                end)

              # Select best candidate
              {best_score, best_node} = Enum.max_by(scored_candidates, fn {score, _} -> score end)

              # Create scheduling decision
              SchedulingDecision.new(
                module_id: module_id,
                node_id: best_node.id,
                score: best_score,
                priority: calculate_priority(module),
                reason: format_reason(best_score, best_node, module),
                factors: calculate_factors(best_node, module, constraints, state.weights),
                execution_policy: opts[:execution_policy] || :immediate
              )
            end
        end
    end
  end

  defp get_module(module_manager, module_id) do
    # Handle both ModuleManager module and pid
    if is_pid(module_manager) or Process.whereis(module_manager) do
      GenServer.call(module_manager, {:get_module, module_id})
    else
      apply(module_manager, :get_module, [module_id])
    end
  end

  defp filter_candidates(nodes, module, constraints, _node_manager) do
    nodes
    |> Enum.filter(&Node.healthy?/1)
    |> Enum.filter(fn node ->
      # Check if module supports the node's architecture
      WasmModule.supports_architecture?(module, node.architecture)
    end)
    |> Enum.filter(fn node ->
      has_capabilities = Node.has_capabilities?(node, constraints.required_capabilities)
      not_excluded = node.id not in constraints.excluded_nodes
      has_capabilities and not_excluded
    end)
    |> Enum.filter(fn node ->
      # Check fault domain constraints
      if constraints.target_fault_domains != [] do
        node.fault_domain in constraints.target_fault_domains
      else
        true
      end
    end)
  end

  defp calculate_score(node, module, constraints, weights) do
    scores = %{
      proximity: score_proximity(node, constraints.preferred_locality),
      resources: score_resources(node, constraints.min_resources),
      fault_domain: score_fault_domain(node),
      architecture: score_architecture(node, module),
      capabilities: score_capabilities(node, module)
    }

    # Weighted sum
    Enum.reduce(scores, 0.0, fn {factor, score}, acc ->
      weight = Map.get(weights, factor, 0.0)
      acc + score * weight
    end)
  end

  # Neutral when no locality preference
  defp score_proximity(_node, nil), do: 0.5

  defp score_proximity(node, preferred_locality) do
    # Score based on geographic/ network proximity
    # In production, this would use actual latency measurements or geo-coordinates
    if node.metadata["region"] == preferred_locality do
      1.0
    else
      # Score decreases with distance (simplified)
      0.3
    end
  end

  defp score_resources(node, min_resources) do
    # Score based on available resources vs requirements
    resource_scores =
      for {resource, required} <- min_resources do
        available = get_in(node.resources, [resource]) || 0

        if available >= required do
          # Bonus for extra capacity, capped at 1.0
          min(1.0, available / (required * 2))
        else
          # Cannot satisfy requirement
          0.0
        end
      end

    if resource_scores == [] do
      # No specific requirements
      1.0
    else
      Enum.sum(resource_scores) / length(resource_scores)
    end
  end

  defp score_fault_domain(node) do
    # Prefer nodes in less populated fault domains for distribution
    # In production, this would check actual node counts per fault domain
    case node.fault_domain do
      "default" -> 0.5
      # Prefer non-default fault domains
      _ -> 0.8
    end
  end

  defp score_architecture(node, module) do
    # Score based on architecture match
    if WasmModule.supports_architecture?(module, node.architecture) do
      1.0
    else
      # Cannot run on incompatible architecture
      0.0
    end
  end

  defp score_capabilities(node, module) do
    # Score based on capability match
    required = module.capabilities
    available = node.capabilities

    if required == [] do
      # No specific capabilities needed
      1.0
    else
      matches = Enum.count(required, &(&1 in available))
      matches / length(required)
    end
  end

  defp calculate_priority(module) do
    # Calculate scheduling priority based on module metadata
    base_priority = 5

    # Boost priority for critical modules
    critical_boost = if module.metadata["critical"] == true, do: 10, else: 0

    # Boost for resource-intensive modules (schedule sooner)
    resource_boost = if module.resource_requirements != %{}, do: 2, else: 0

    base_priority + critical_boost + resource_boost
  end

  defp calculate_factors(node, module, constraints, weights) do
    %{
      proximity_score: score_proximity(node, constraints.preferred_locality),
      resource_score: score_resources(node, constraints.min_resources),
      fault_domain: node.fault_domain,
      architecture: node.architecture,
      capabilities_match: score_capabilities(node, module),
      weights_applied: weights
    }
  end

  defp format_reason(score, node, module) do
    reasons = []
    reasons = if score > 0.9, do: ["optimal placement" | reasons], else: reasons
    reasons = if node.status == :online, do: ["healthy node" | reasons], else: reasons

    reasons =
      if WasmModule.supports_architecture?(module, node.architecture),
        do: ["architecture compatible" | reasons],
        else: reasons

    "Selected #{node.hostname} (score: #{Float.round(score, 2)}) - #{Enum.join(reasons, ", ")}"
  end

  defp update_stats(state, decision_time_ms) do
    total = state.stats.total_scheduled + 1
    avg = (state.stats.avg_decision_time_ms * (total - 1) + decision_time_ms) / total

    %{
      state
      | stats: %{state.stats | total_scheduled: total, avg_decision_time_ms: Float.round(avg, 3)}
    }
  end
end
