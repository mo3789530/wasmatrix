defmodule WasmatrixControl.Models.WasmModuleTest do
  use ExUnit.Case
  alias WasmatrixControl.Models.WasmModule

  describe "WasmModule.new/1" do
    test "creates a valid module with required fields" do
      assert {:ok, mod} = WasmModule.new(name: "test-module", hash: "a" |> String.duplicate(64))
      assert mod.name == "test-module"
      assert mod.version == "1.0.0"
      assert mod.id =~ ~r/^mod-/
      assert %DateTime{} = mod.created_at
      assert %DateTime{} = mod.updated_at
    end

    test "accepts custom id" do
      assert {:ok, mod} =
               WasmModule.new(id: "custom-id", name: "test", hash: "a" |> String.duplicate(64))

      assert mod.id == "custom-id"
    end

    test "returns error for invalid hash format" do
      assert {:error, errors} = WasmModule.new(name: "test", hash: "invalid")
      assert "hash must be a valid SHA-256 hex string" in errors

      assert {:error, errors} = WasmModule.new(name: "test", hash: "")
      assert "hash is required" in errors
    end

    test "validates resource requirements" do
      attrs = [
        name: "test",
        hash: "a" |> String.duplicate(64),
        resource_requirements: %{cpu: 2, memory: 1024}
      ]

      assert {:ok, mod} = WasmModule.new(attrs)
      assert mod.resource_requirements.cpu == 2
    end
  end

  describe "WasmModule.new_version/2" do
    test "creates a new version from existing module" do
      {:ok, original} =
        WasmModule.new(
          name: "test",
          hash: "a" |> String.duplicate(64),
          version: "1.0.0"
        )

      assert {:ok, new_version} =
               WasmModule.new_version(original, hash: "b" |> String.duplicate(64))

      assert new_version.name == original.name
      assert new_version.version == "1.0.1"
      assert new_version.id != original.id
      assert new_version.created_at == original.created_at
      assert new_version.updated_at != original.updated_at
    end

    test "allows overriding specific fields" do
      {:ok, original} = WasmModule.new(name: "test", hash: "a" |> String.duplicate(64))

      assert {:ok, new_version} =
               WasmModule.new_version(original,
                 version: "2.0.0",
                 capabilities: ["new-cap"]
               )

      assert new_version.version == "2.0.0"
      assert new_version.capabilities == ["new-cap"]
    end
  end

  describe "WasmModule.supports_architecture?/2" do
    test "returns true for supported architectures" do
      {:ok, mod} =
        WasmModule.new(
          name: "test",
          hash: "a" |> String.duplicate(64),
          architecture: ["x86_64", "arm64"]
        )

      assert WasmModule.supports_architecture?(mod, "x86_64")
      assert WasmModule.supports_architecture?(mod, "arm64")
      refute WasmModule.supports_architecture?(mod, "riscv64")
    end

    test "returns true for 'any' architecture" do
      {:ok, mod} =
        WasmModule.new(
          name: "test",
          hash: "a" |> String.duplicate(64),
          architecture: ["any"]
        )

      assert WasmModule.supports_architecture?(mod, "x86_64")
    end
  end

  describe "WasmModule.has_capabilities?/2" do
    test "returns true when all capabilities present" do
      {:ok, mod} =
        WasmModule.new(
          name: "test",
          hash: "a" |> String.duplicate(64),
          capabilities: ["http", "filesystem"]
        )

      assert WasmModule.has_capabilities?(mod, ["http"])
      assert WasmModule.has_capabilities?(mod, ["http", "filesystem"])
    end

    test "returns false when capabilities missing" do
      {:ok, mod} =
        WasmModule.new(
          name: "test",
          hash: "a" |> String.duplicate(64),
          capabilities: ["http"]
        )

      refute WasmModule.has_capabilities?(mod, ["http", "database"])
    end
  end

  describe "WasmModule.compatible?/2" do
    test "returns true for same name and compatible architecture" do
      {:ok, mod1} =
        WasmModule.new(name: "test", hash: "a" |> String.duplicate(64), architecture: ["x86_64"])

      {:ok, mod2} =
        WasmModule.new(name: "test", hash: "b" |> String.duplicate(64), architecture: ["x86_64"])

      assert WasmModule.compatible?(mod1, mod2)
    end

    test "returns false for different names" do
      {:ok, mod1} = WasmModule.new(name: "test1", hash: "a" |> String.duplicate(64))
      {:ok, mod2} = WasmModule.new(name: "test2", hash: "b" |> String.duplicate(64))

      refute WasmModule.compatible?(mod1, mod2)
    end
  end

  describe "JSON encoding" do
    test "encodes to JSON with ISO8601 timestamps" do
      {:ok, mod} = WasmModule.new(name: "test", hash: "a" |> String.duplicate(64))
      json = Jason.encode!(mod)
      assert is_binary(json)
      assert json =~ "\"name\":\"test\""
    end
  end
end
