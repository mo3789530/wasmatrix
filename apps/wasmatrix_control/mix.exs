defmodule WasmatrixControl.MixProject do
  use Mix.Project

  def project do
    [
      app: :wasmatrix_control,
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
      extra_applications: [:logger, :observer, :wx, :runtime_tools],
      mod: {WasmatrixControl.Application, []}
    ]
  end

  defp deps do
    [
      # Communication
      {:phoenix_pubsub, "~> 2.1"},
      {:gen_state_machine, "~> 3.0"},
      {:mint, "~> 1.5"},
      {:castore, "~> 1.0"},
      {:jason, "~> 1.4"},
      {:protobuf, "~> 0.12"},

      # Scheduling
      {:libgraph, "~> 0.16"},
      {:geo, "~> 3.5"},

      # State management
      {:cubdb, "~> 2.0"},
      {:delta_crdt, "~> 0.6"},

      # Security
      {:ex_crypto, "~> 0.10"},
      {:ex_hash_ring, "~> 6.0"},

      # Testing
      {:ex_unit_clustered_case, "~> 0.5", only: :test},
      {:propcheck, "~> 1.4", only: [:dev, :test]},
      {:stream_data, "~> 0.6", only: [:dev, :test]}
    ]
  end
end
