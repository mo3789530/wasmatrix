defmodule WasmatrixWeb.MixProject do
  use Mix.Project

  def project do
    [
      app: :wasmatrix_web,
      version: "0.1.0",
      build_path: "../../_build",
      config_path: "../../config/config.exs",
      deps_path: "../../deps",
      lockfile: "../../mix.lock",
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {WasmatrixWeb.Application, []}
    ]
  end

  defp deps do
    [
      {:wasmatrix_control, in_umbrella: true},

      # Web API
      {:phoenix, "~> 1.7"},
      {:phoenix_ecto, "~> 4.4"},
      {:phoenix_live_dashboard, "~> 0.8"},
      {:telemetry_metrics, "~> 1.0"},
      {:telemetry_poller, "~> 1.0"},
      {:jason, "~> 1.4"},
      {:plug_cowboy, "~> 2.6"}

      # gRPC (disabled - requires additional setup)
      # {:grpc, "~> 0.8"},

      # MQTT/NATS (disabled - requires cmake)
      # {:emqtt, "~> 1.8"},
      # {:gnat, "~> 1.8"}
    ]
  end
end
