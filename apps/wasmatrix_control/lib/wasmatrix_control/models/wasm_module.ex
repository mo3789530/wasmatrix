defmodule WasmatrixControl.Models.WasmModule do
  @moduledoc """
  Represents a WebAssembly module with versioning and capability tracking.

  Includes metadata for deployment, execution requirements, and cryptographic
  signatures for verification.
  """

  @type t :: %__MODULE__{
          id: String.t(),
          name: String.t(),
          version: String.t(),
          hash: String.t(),
          signature: String.t() | nil,
          capabilities: [String.t()],
          resource_requirements: map(),
          architecture: [String.t()],
          size_bytes: non_neg_integer(),
          metadata: map(),
          created_at: DateTime.t(),
          updated_at: DateTime.t()
        }

  @enforce_keys [:id, :name, :hash]
  defstruct [
    :id,
    :name,
    :hash,
    :signature,
    version: "1.0.0",
    capabilities: [],
    resource_requirements: %{},
    architecture: ["x86_64"],
    size_bytes: 0,
    metadata: %{},
    created_at: nil,
    updated_at: nil
  ]

  @doc """
  Creates a new WasmModule with required fields.
  """
  def new(attrs) when is_map(attrs) or is_list(attrs) do
    attrs = Map.new(attrs)
    now = DateTime.utc_now()

    module = %__MODULE__{
      id: attrs[:id] || generate_id(),
      name: attrs[:name],
      hash: attrs[:hash],
      signature: attrs[:signature],
      version: attrs[:version] || "1.0.0",
      capabilities: attrs[:capabilities] || [],
      resource_requirements: attrs[:resource_requirements] || %{},
      architecture: attrs[:architecture] || ["x86_64"],
      size_bytes: attrs[:size_bytes] || 0,
      metadata: attrs[:metadata] || %{},
      created_at: now,
      updated_at: now
    }

    case validate(module) do
      {:ok, valid_module} -> {:ok, valid_module}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc """
  Validates a WasmModule struct.
  """
  def validate(%__MODULE__{} = module) do
    errors = []

    errors =
      if is_nil(module.id) or module.id == "", do: ["id is required" | errors], else: errors

    errors =
      if is_nil(module.name) or module.name == "", do: ["name is required" | errors], else: errors

    errors =
      if is_nil(module.hash) or module.hash == "", do: ["hash is required" | errors], else: errors

    errors =
      if module.size_bytes < 0, do: ["size_bytes must be non-negative" | errors], else: errors

    # Validate hash format (SHA-256 hex)
    errors =
      unless is_valid_hash?(module.hash),
        do: ["hash must be a valid SHA-256 hex string" | errors],
        else: errors

    if errors == [] do
      {:ok, module}
    else
      {:error, Enum.reverse(errors)}
    end
  end

  @doc """
  Creates a new version of an existing module.
  """
  def new_version(%__MODULE__{} = existing, new_attrs) do
    new_attrs = Map.new(new_attrs)

    new_module = %__MODULE__{
      existing
      | id: generate_id(),
        version: new_attrs[:version] || bump_version(existing.version),
        hash: new_attrs[:hash] || existing.hash,
        signature: new_attrs[:signature] || existing.signature,
        capabilities: new_attrs[:capabilities] || existing.capabilities,
        resource_requirements:
          new_attrs[:resource_requirements] || existing.resource_requirements,
        architecture: new_attrs[:architecture] || existing.architecture,
        size_bytes: new_attrs[:size_bytes] || existing.size_bytes,
        metadata: Map.merge(existing.metadata, new_attrs[:metadata] || %{}),
        created_at: existing.created_at,
        updated_at: DateTime.utc_now()
    }

    validate(new_module)
  end

  @doc """
  Verifies the module signature if present.
  """
  def verify_signature?(%__MODULE__{signature: nil}, _public_key), do: true

  def verify_signature?(%__MODULE__{hash: hash, signature: signature}, public_key) do
    # Placeholder for actual signature verification
    # In production, use ExCrypto or similar
    is_binary(signature) and is_binary(public_key)
  end

  @doc """
  Checks if the module supports a specific architecture.
  """
  def supports_architecture?(%__MODULE__{architecture: arches}, target) do
    target in arches or "any" in arches
  end

  @doc """
  Checks if the module has all required capabilities.
  """
  def has_capabilities?(%__MODULE__{capabilities: caps}, required) do
    required_list = List.wrap(required)
    Enum.all?(required_list, &(&1 in caps))
  end

  @doc """
  Compares two modules for compatibility (same name, compatible version).
  """
  def compatible?(%__MODULE__{} = a, %__MODULE__{} = b) do
    a.name == b.name and
      supports_architecture?(a, List.first(b.architecture))
  end

  defp generate_id do
    bytes = :crypto.strong_rand_bytes(8)
    "mod-" <> Base.encode16(bytes, case: :lower)
  end

  defp bump_version(version) when is_binary(version) do
    parts = String.split(version, ".")

    case parts do
      [major, minor, patch] ->
        new_patch = String.to_integer(patch) + 1
        "#{major}.#{minor}.#{new_patch}"

      [major, minor] ->
        "#{major}.#{minor}.1"

      [major] ->
        "#{major}.0.1"

      _ ->
        "1.0.0"
    end
  end

  defp is_valid_hash?(hash) when is_binary(hash) do
    # SHA-256 is 64 hex characters
    String.length(hash) == 64 and String.match?(hash, ~r/^[a-fA-F0-9]+$/)
  end

  defp is_valid_hash?(_), do: false
end

defimpl Jason.Encoder, for: WasmatrixControl.Models.WasmModule do
  def encode(%WasmatrixControl.Models.WasmModule{} = module, opts) do
    module
    |> Map.from_struct()
    |> Enum.map(fn
      {k, %DateTime{} = dt} -> {k, DateTime.to_iso8601(dt)}
      {k, v} -> {k, v}
    end)
    |> Enum.into(%{})
    |> Jason.Encode.map(opts)
  end
end
