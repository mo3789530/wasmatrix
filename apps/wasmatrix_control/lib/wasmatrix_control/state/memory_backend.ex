defmodule WasmatrixControl.State.MemoryBackend do
  @moduledoc """
  In-memory backend for StateManager (for testing and single-node use).
  """

  @type state :: %{
          data: %{String.t() => term()}
        }

  def init(_opts) do
    {:ok, %{data: %{}}}
  end

  def get(key, state) do
    case Map.get(state.data, key) do
      nil -> {:error, :not_found}
      value -> {:ok, value, state}
    end
  end

  def put(key, value, state) do
    new_state = %{state | data: Map.put(state.data, key, value)}
    {:ok, new_state}
  end

  def delete(key, state) do
    new_state = %{state | data: Map.delete(state.data, key)}
    {:ok, new_state}
  end

  def keys(state) do
    {:ok, Map.keys(state.data), state}
  end
end
