defmodule WasmatrixControl.Models.SchedulingDecision do
  @moduledoc """
  Represents a scheduling decision made by the ProximityScheduler.

  Contains the target node, selected module version, priority score,
  and reasoning for the placement decision.
  """

  @type t :: %__MODULE__{
          id: String.t(),
          module_id: String.t(),
          node_id: String.t(),
          priority: non_neg_integer(),
          score: float(),
          reason: String.t(),
          factors: map(),
          timestamp: DateTime.t(),
          valid_until: DateTime.t() | nil,
          status: :pending | :accepted | :rejected | :expired,
          execution_policy: :immediate | :queued | :deferred
        }

  @enforce_keys [:module_id, :node_id]
  defstruct [
    :id,
    :module_id,
    :node_id,
    priority: 0,
    score: 0.0,
    reason: "",
    factors: %{},
    timestamp: nil,
    valid_until: nil,
    status: :pending,
    execution_policy: :immediate
  ]

  @doc """
  Creates a new scheduling decision.
  """
  def new(attrs) when is_map(attrs) or is_list(attrs) do
    attrs = Map.new(attrs)
    now = DateTime.utc_now()

    decision = %__MODULE__{
      id: attrs[:id] || generate_id(),
      module_id: attrs[:module_id],
      node_id: attrs[:node_id],
      priority: attrs[:priority] || 0,
      score: attrs[:score] || 0.0,
      reason: attrs[:reason] || "",
      factors: attrs[:factors] || %{},
      timestamp: now,
      valid_until: attrs[:valid_until] || DateTime.add(now, 60, :second),
      status: attrs[:status] || :pending,
      execution_policy: attrs[:execution_policy] || :immediate
    }

    case validate(decision) do
      {:ok, valid_decision} -> {:ok, valid_decision}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc """
  Validates a scheduling decision.
  """
  def validate(%__MODULE__{} = decision) do
    errors = []

    errors =
      if is_nil(decision.module_id) or decision.module_id == "",
        do: ["module_id is required" | errors],
        else: errors

    errors =
      if is_nil(decision.node_id) or decision.node_id == "",
        do: ["node_id is required" | errors],
        else: errors

    errors =
      if decision.priority < 0, do: ["priority must be non-negative" | errors], else: errors

    errors =
      if decision.score < 0.0 or decision.score > 1.0,
        do: ["score must be between 0.0 and 1.0" | errors],
        else: errors

    errors =
      unless decision.status in [:pending, :accepted, :rejected, :expired],
        do: ["invalid status" | errors],
        else: errors

    errors =
      unless decision.execution_policy in [:immediate, :queued, :deferred],
        do: ["invalid execution_policy" | errors],
        else: errors

    if errors == [] do
      {:ok, decision}
    else
      {:error, Enum.reverse(errors)}
    end
  end

  @doc """
  Marks the decision as accepted.
  """
  def accept(%__MODULE__{} = decision) do
    %{decision | status: :accepted}
  end

  @doc """
  Marks the decision as rejected with a reason.
  """
  def reject(%__MODULE__{} = decision, reason \\ "") do
    %{decision | status: :rejected, reason: reason}
  end

  @doc """
  Checks if the decision has expired.
  """
  def expired?(%__MODULE__{valid_until: nil}), do: false

  def expired?(%__MODULE__{valid_until: valid_until}) do
    DateTime.compare(DateTime.utc_now(), valid_until) == :gt
  end

  @doc """
  Updates the decision status to expired if past validity.
  """
  def check_expiration(%__MODULE__{} = decision) do
    if expired?(decision) do
      %{decision | status: :expired}
    else
      decision
    end
  end

  @doc """
  Calculates decision quality based on score and factors.
  """
  def quality_score(%__MODULE__{score: score, factors: factors}) do
    factor_bonus = map_size(factors) * 0.01
    min(score + factor_bonus, 1.0)
  end

  defp generate_id do
    bytes = :crypto.strong_rand_bytes(8)
    "sched-" <> Base.encode16(bytes, case: :lower)
  end
end

defimpl Jason.Encoder, for: WasmatrixControl.Models.SchedulingDecision do
  def encode(%WasmatrixControl.Models.SchedulingDecision{} = decision, opts) do
    decision
    |> Map.from_struct()
    |> Enum.map(fn
      {k, %DateTime{} = dt} -> {k, DateTime.to_iso8601(dt)}
      {k, v} -> {k, v}
    end)
    |> Enum.into(%{})
    |> Jason.Encode.map(opts)
  end
end
