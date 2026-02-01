defmodule WasmatrixWeb.Application do
  @moduledoc """
  Web interface and API application for Wasmatrix.
  """
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      # Web server and endpoints will be added here
    ]

    opts = [strategy: :one_for_one, name: WasmatrixWeb.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
