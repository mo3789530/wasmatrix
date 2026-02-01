defmodule WasmatrixControl.Events.LocalTransport do
  @moduledoc """
  Local in-memory transport for EventSystem.

  Used for testing and single-node deployments.
  """

  @type state :: %{
          messages: [map()]
        }

  @spec init(keyword()) :: {:ok, state()}
  def init(_opts) do
    {:ok, %{messages: []}}
  end

  @spec publish(map(), state()) :: {:ok, state()} | {:error, term(), state()}
  def publish(event, state) do
    # In local transport, we just store the message
    new_state = %{state | messages: [event | state.messages]}
    {:ok, new_state}
  end

  @spec get_messages(state()) :: [map()]
  def get_messages(state) do
    state.messages
  end

  @spec clear(state()) :: state()
  def clear(state) do
    %{state | messages: []}
  end
end
