defmodule WasmatrixControl.ModuleManagerTest do
  use ExUnit.Case
  alias WasmatrixControl.ModuleManager
  alias WasmatrixControl.Models.WasmModule

  setup do
    {:ok, pid} = ModuleManager.start_link(name: :test_module_manager)
    {:ok, manager: pid}
  end

  describe "upload_module/3" do
    test "uploads a new module successfully", %{manager: manager} do
      # WASM magic bytes
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      attrs = [
        name: "test-module",
        capabilities: ["http", "filesystem"],
        architecture: ["x86_64"],
        signature: "test-signature"
      ]

      assert {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)
      assert module.name == "test-module"
      assert module.size_bytes == 4
      assert module.hash == :crypto.hash(:sha256, binary) |> Base.encode16(case: :lower)
    end

    test "prevents duplicate hash uploads", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D>>
      attrs = [name: "test-module"]

      assert {:ok, module1} = ModuleManager.upload_module(manager, attrs, binary)

      assert {:error, {:duplicate_hash, ^module1}} =
               ModuleManager.upload_module(manager, attrs, binary)
    end

    test "uploads without binary", %{manager: manager} do
      attrs = [
        name: "test-module",
        hash: String.duplicate("a", 64)
      ]

      assert {:ok, module} = ModuleManager.upload_module(manager, attrs)
      assert module.name == "test-module"
      # Binary will be nil
      assert {:error, :binary_not_found} = ModuleManager.get_binary(manager, module.id)
    end
  end

  describe "get_module/2" do
    test "returns module by id", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D>>
      attrs = [name: "test-module"]

      {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)
      assert {:ok, ^module} = ModuleManager.get_module(manager, module.id)
    end

    test "returns error for unknown module", %{manager: manager} do
      assert {:error, :not_found} = ModuleManager.get_module(manager, "unknown-id")
    end
  end

  describe "get_binary/2" do
    test "returns binary and module", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D, 0x01, 0x02, 0x03>>
      attrs = [name: "test-module"]

      {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)
      assert {:ok, ^binary, ^module} = ModuleManager.get_binary(manager, module.id)
    end

    test "returns error when binary not stored", %{manager: manager} do
      attrs = [name: "test-module", hash: String.duplicate("a", 64)]
      {:ok, module} = ModuleManager.upload_module(manager, attrs)

      assert {:error, :binary_not_found} = ModuleManager.get_binary(manager, module.id)
    end
  end

  describe "list_modules/1" do
    test "returns all modules", %{manager: manager} do
      binary1 = <<0x00>>
      binary2 = <<0x01>>

      {:ok, mod1} = ModuleManager.upload_module(manager, [name: "module-1"], binary1)
      {:ok, mod2} = ModuleManager.upload_module(manager, [name: "module-2"], binary2)

      {:ok, modules} = ModuleManager.list_modules(manager)
      assert length(modules) == 2
      assert mod1.id in Enum.map(modules, & &1.id)
      assert mod2.id in Enum.map(modules, & &1.id)
    end
  end

  describe "list_versions/2" do
    test "returns all versions of a module", %{manager: manager} do
      binary1 = <<0x00>>
      binary2 = <<0x01>>

      {:ok, v1} =
        ModuleManager.upload_module(manager, [name: "my-module", version: "1.0.0"], binary1)

      {:ok, v2} =
        ModuleManager.upload_module(manager, [name: "my-module", version: "1.1.0"], binary2)

      {:ok, versions} = ModuleManager.list_versions(manager, "my-module")
      assert length(versions) == 2
      # Newest first
      assert hd(versions).id == v2.id
    end

    test "returns empty list for unknown module", %{manager: manager} do
      {:ok, versions} = ModuleManager.list_versions(manager, "unknown")
      assert versions == []
    end
  end

  describe "get_latest_version/2" do
    test "returns the latest version", %{manager: manager} do
      binary1 = <<0x00>>
      binary2 = <<0x01>>

      {:ok, _v1} =
        ModuleManager.upload_module(manager, [name: "my-module", version: "1.0.0"], binary1)

      {:ok, v2} =
        ModuleManager.upload_module(manager, [name: "my-module", version: "1.1.0"], binary2)

      assert {:ok, latest} = ModuleManager.get_latest_version(manager, "my-module")
      assert latest.id == v2.id
      assert latest.version == "1.1.0"
    end

    test "returns error for unknown module", %{manager: manager} do
      assert {:error, :not_found} = ModuleManager.get_latest_version(manager, "unknown")
    end
  end

  describe "verify_signature/3" do
    test "verifies module with signature", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      attrs = [
        name: "signed-module",
        signature: "valid-signature"
      ]

      {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)

      # Placeholder verification always returns true for now
      assert {:ok, true} = ModuleManager.verify_signature(manager, module.id, "public-key")
    end

    test "returns error for module without signature", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D>>
      attrs = [name: "unsigned-module"]

      {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)
      assert {:error, :no_signature} = ModuleManager.verify_signature(manager, module.id, "key")
    end
  end

  describe "deploy_to_node/3" do
    test "deploys module to node", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D>>
      attrs = [name: "deployable-module"]

      {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)

      assert {:ok, result} = ModuleManager.deploy_to_node(manager, module.id, "node-123")
      assert result.module.id == module.id
      assert result.node_id == "node-123"
      assert result.size == byte_size(binary)
    end

    test "returns error for unknown module", %{manager: manager} do
      assert {:error, :not_found} = ModuleManager.deploy_to_node(manager, "unknown", "node-123")
    end
  end

  describe "rollback/3" do
    test "rolls back to previous version", %{manager: manager} do
      binary1 = <<0x00>>
      binary2 = <<0x01>>

      {:ok, v1} =
        ModuleManager.upload_module(manager, [name: "rollback-test", version: "1.0.0"], binary1)

      {:ok, _v2} =
        ModuleManager.upload_module(manager, [name: "rollback-test", version: "1.1.0"], binary2)

      # Rollback to v1
      assert {:ok, result} = ModuleManager.rollback(manager, "rollback-test", "1.0.0")
      assert result.module.id == v1.id
      assert result.module.version == "1.0.0"
      assert result.elapsed_ms >= 0

      # Verify latest is now v1
      assert {:ok, latest} = ModuleManager.get_latest_version(manager, "rollback-test")
      assert latest.id == v1.id
    end

    test "completes rollback within 10ms", %{manager: manager} do
      binary1 = <<0x00>>
      binary2 = <<0x01>>

      ModuleManager.upload_module(manager, [name: "perf-test", version: "1.0.0"], binary1)
      ModuleManager.upload_module(manager, [name: "perf-test", version: "1.1.0"], binary2)

      assert {:ok, result} = ModuleManager.rollback(manager, "perf-test", "1.0.0")
      assert result.elapsed_ms < 10, "Rollback took #{result.elapsed_ms}ms, expected < 10ms"
    end

    test "returns error for unknown version", %{manager: manager} do
      assert {:error, :no_versions} = ModuleManager.rollback(manager, "unknown", "1.0.0")
    end
  end

  describe "delete_module/2" do
    test "deletes module and binary", %{manager: manager} do
      binary = <<0x00, 0x61, 0x73, 0x6D>>
      attrs = [name: "deletable-module"]

      {:ok, module} = ModuleManager.upload_module(manager, attrs, binary)
      assert {:ok, ^module} = ModuleManager.delete_module(manager, module.id)

      # Verify deletion
      assert {:error, :not_found} = ModuleManager.get_module(manager, module.id)
      assert {:error, :module_not_found} = ModuleManager.get_binary(manager, module.id)
    end

    test "returns error for unknown module", %{manager: manager} do
      assert {:error, :not_found} = ModuleManager.delete_module(manager, "unknown")
    end
  end
end
