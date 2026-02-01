import Config

config :wasmatrix_control,
  node_heartbeat_interval: 5_000,
  node_timeout: 15_000,
  scheduler_cache_ttl: 1_000

config :wasmatrix_web,
  http_port: 4000,
  grpc_port: 50051

config :logger,
  level: :info,
  format: "$time $metadata[$level] $message\n"

import_config "#{config_env()}.exs"
