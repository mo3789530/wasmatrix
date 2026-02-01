defmodule WasmatrixControl.ModuleManager do
  @moduledoc """
  GenServer for managing WebAssembly modules with versioning and distribution.

  Handles:
  - Wasm module storage with version history
   Cryptographic signature verification system
  - Module capability definition and validation
  - Module deployment to target nodes
  - Rollback functionality with 10ms performance target

  Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3, 3.4, 3.5
  """

  use GenServer

  alias WasmatrixControl.Models.WasmModule
  alias WasmatrixControl.Models.EventMessage

  @type state :: %{
          # module_id -> module
          modules: %{String.t() => WasmModule.t()},
          # name -> [module_id, ...] (newest first)
          versions: %{String.t() => [String.t()]},
          # module_id -> wasm_binary
          binaries: %{String.t() => binary()},
          # module_id -> signature_info
          signatures: %{String.t() => map()}
        }

  # Client API

  def start_link(opts \\ []) do
    name = opts[:name] || __MODULE__
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  @doc """
  Uploads a new WebAssembly module.
  """
  def upload_module(server \\ __MODULE__, attrs, binary \\ nil) do
    GenServer.call(server, {:upload, attrs, binary})
  end

  @doc """
  Gets a module by ID.
  """
  def get_module(server \\ __MODULE__, module_id) do
    GenServer.call(server, {:get_module, module_id})
  end

  @doc """
  Gets module binary by ID.
  """
  def get_binary(server \\ __MODULE__, module_id) do
    GenServer.call(server, {:get_binary, module_id})
  end

  @doc """
  Lists all modules.
  """
  def list_modules(server \\ __MODULE__) do
    GenServer.call(server, :list_modules)
  end

  @doc """
  Lists all versions of a module by name.
  """
  def list_versions(server \\ __MODULE__, module_name) do
    GenServer.call(server, {:list_versions, module_name})
  end

  @doc """
  Gets the latest version of a module.
  """
  def get_latest_version(server \\ __MODULE__, module_name) do
    GenServer.call(server, {:get_latest, module_name})
  end

  @doc """
  Verifies module signature.
  """
  def verify_signature(server \\ __MODULE__, module_id, public_key) do
    GenServer.call(server, {:verify_signature, module_id, public_key})
  end

  @doc """
  Deploys a module to a specific node.
  """
  def deploy_to_node(server \\ __MODULE__, module_id, node_id) do
    GenServer.call(server, {:deploy, module_id, node_id})
  end

  @doc """
  Rolls back to a previous version.
  """
  def rollback(server \\ __MODULE__, module_name, target_version) do
    GenServer.call(server, {:rollback, module_name, target_version})
  end

  @doc """
  Deletes a module.
  """
  def delete_module(server \\ __MODULE__, module_id) do
    GenServer.call(server, {:delete, module_id})
  end

  # Server Callbacks

  @impl true
  def init(_opts) do
    state = %{
      modules: %{},
      versions: %{},
      binaries: %{},
      signatures: %{}
    }

    {:ok, state}
  end

  @impl true
  def handle_call({:upload, attrs, binary}, _from, state) do
    # Convert keyword list to map if needed
    attrs = Map.new(attrs)

    # Calculate hash from binary if provided
    attrs =
      if binary do
        hash = calculate_hash(binary)
        Map.merge(attrs, %{hash: hash, size_bytes: byte_size(binary)})
      else
        attrs
      end

    case WasmModule.new(attrs) do
      {:ok, module} ->
        # Check for version conflicts
        existing_versions = Map.get(state.versions, module.name, [])

        # Check if this exact hash already exists
        if existing_id = find_by_hash(existing_versions, module.hash, state.modules) do
          existing_module = Map.get(state.modules, existing_id)
          {:reply, {:error, {:duplicate_hash, existing_module}}, state}
        else
          # Store module
          new_state =
            state
            |> put_in([:modules, module.id], module)
            |> put_in([:versions, module.name], [module.id | existing_versions])

          # Store binary if provided
          new_state =
            if binary do
              put_in(new_state, [:binaries, module.id], binary)
            else
              new_state
            end

          # Store signature info if provided
          new_state =
            if module.signature do
              put_in(new_state, [:signatures, module.id], %{
                signature: module.signature,
                # Default, can be made configurable
                algorithm: "ed25519",
                timestamp: DateTime.utc_now()
              })
            else
              new_state
            end

          # Publish event
          {:ok, event} =
            EventMessage.new(
              type: "module.uploaded",
              source: "module_manager",
              payload: %{
                module_id: module.id,
                name: module.name,
                version: module.version
              }
            )

          publish_event(event)

          {:reply, {:ok, module}, new_state}
        end

      {:error, reason} ->
        {:reply, {:error, reason}, state}
    end
  end

  @impl true
  def handle_call({:get_module, module_id}, _from, state) do
    reply =
      case Map.get(state.modules, module_id) do
        nil -> {:error, :not_found}
        module -> {:ok, module}
      end

    {:reply, reply, state}
  end

  @impl true
  def handle_call({:get_binary, module_id}, _from, state) do
    reply =
      cond do
        module = state.modules[module_id] ->
          case Map.get(state.binaries, module_id) do
            nil -> {:error, :binary_not_found}
            binary -> {:ok, binary, module}
          end

        true ->
          {:error, :module_not_found}
      end

    {:reply, reply, state}
  end

  @impl true
  def handle_call(:list_modules, _from, state) do
    modules = Map.values(state.modules)
    {:reply, {:ok, modules}, state}
  end

  @impl true
  def handle_call({:list_versions, module_name}, _from, state) do
    version_ids = Map.get(state.versions, module_name, [])

    versions =
      Enum.map(version_ids, &Map.get(state.modules, &1))
      |> Enum.reject(&is_nil/1)

    {:reply, {:ok, versions}, state}
  end

  @impl true
  def handle_call({:get_latest, module_name}, _from, state) do
    reply =
      case Map.get(state.versions, module_name, []) do
        [] ->
          {:error, :not_found}

        [latest_id | _] ->
          case Map.get(state.modules, latest_id) do
            nil -> {:error, :not_found}
            module -> {:ok, module}
          end
      end

    {:reply, reply, state}
  end

  @impl true
  def handle_call({:verify_signature, module_id, public_key}, _from, state) do
    reply =
      with {:ok, module} <- Map.fetch(state.modules, module_id),
           sig_info when not is_nil(sig_info) <- Map.get(state.signatures, module_id) do
        # Placeholder for actual signature verification
        # In production, use proper crypto library
        verified = WasmModule.verify_signature?(module, public_key)
        {:ok, verified}
      else
        :error -> {:error, :module_not_found}
        nil -> {:error, :no_signature}
      end

    {:reply, reply, state}
  end

  @impl true
  def handle_call({:deploy, module_id, node_id}, _from, state) do
    reply =
      with {:ok, binary, module} <- get_binary_internal(state, module_id) do
        # In production, this would:
        # 1. Stream binary to target node
        # 2. Verify on target node
        # 3. Trigger module instantiation

        # Publish deployment event
        {:ok, event} =
          EventMessage.module_deployed(module_id, node_id, %{
            timestamp: DateTime.utc_now()
          })

        publish_event(event)

        {:ok, %{module: module, node_id: node_id, size: byte_size(binary)}}
      end

    {:reply, reply, state}
  end

  @impl true
  def handle_call({:rollback, module_name, target_version}, _from, state) do
    # Start timing for performance check
    start_time = System.monotonic_time(:millisecond)

    version_ids = Map.get(state.versions, module_name)

    cond do
      is_nil(version_ids) or version_ids == [] ->
        {:reply, {:error, :no_versions}, state}

      true ->
        target_id = find_version_id(version_ids, target_version, state.modules)

        if is_nil(target_id) do
          {:reply, {:error, :version_not_found}, state}
        else
          target_module = Map.get(state.modules, target_id)

          if is_nil(target_module) do
            {:reply, {:error, :version_not_found}, state}
          else
            # Mark this version as the active one by moving it to front
            new_versions = [target_id | List.delete(version_ids, target_id)]
            new_state = put_in(state, [:versions, module_name], new_versions)

            # Calculate elapsed time
            elapsed = System.monotonic_time(:millisecond) - start_time

            # Publish rollback event
            {:ok, event} =
              EventMessage.new(
                type: "module.rollback",
                source: "module_manager",
                payload: %{
                  module_name: module_name,
                  version: target_version,
                  module_id: target_id,
                  elapsed_ms: elapsed
                }
              )

            publish_event(event)

            {:reply, {:ok, %{module: target_module, elapsed_ms: elapsed}}, new_state}
          end
        end
    end
  end

  @impl true
  def handle_call({:delete, module_id}, _from, state) do
    case Map.get(state.modules, module_id) do
      nil ->
        {:reply, {:error, :not_found}, state}

      module ->
        # Remove from all data structures
        new_state =
          state
          |> update_in([:modules], &Map.delete(&1, module_id))
          |> update_in([:binaries], &Map.delete(&1, module_id))
          |> update_in([:signatures], &Map.delete(&1, module_id))
          |> update_in([:versions, module.name], &List.delete(&1 || [], module_id))

        {:reply, {:ok, module}, new_state}
    end
  end

  # Private Functions

  defp calculate_hash(binary) do
    :crypto.hash(:sha256, binary) |> Base.encode16(case: :lower)
  end

  defp find_by_hash(version_ids, hash, modules) do
    Enum.find_value(version_ids, fn id ->
      case Map.get(modules, id) do
        %{hash: ^hash} -> id
        _ -> nil
      end
    end)
  end

  defp find_version_id(version_ids, target_version, modules) do
    Enum.find(version_ids, fn id ->
      case Map.get(modules, id) do
        %{version: ^target_version} -> true
        _ -> false
      end
    end)
  end

  defp get_binary_internal(state, module_id) do
    with module when not is_nil(module) <- Map.get(state.modules, module_id),
         binary when not is_nil(binary) <- Map.get(state.binaries, module_id) do
      {:ok, binary, module}
    else
      nil -> {:error, :not_found}
    end
  end

  defp publish_event(%EventMessage{} = _event) do
    # Placeholder for EventSystem integration (Task 7)
    :ok
  end
end
