defmodule WasmatrixControl.Application do
  @moduledoc """
  Main application supervisor for Wasmatrix control plane.
  """
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      # Core data models and managers
      WasmatrixControl.NodeManager,
      WasmatrixControl.ModuleManager,
      WasmatrixControl.Scheduler.ProximityScheduler,
      WasmatrixControl.Events.EventSystem
      # Note: StateManager is dynamically started in tests
    ]

    opts = [strategy: :one_for_one, name: WasmatrixControl.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
