import Config

config :logger,
  level: :info,
  format: "$time $metadata[$level] $message\n"

import_config "#{config_env()}.exs"
