defmodule WasmatrixControl.Models.NodeTest do
  use ExUnit.Case
  alias WasmatrixControl.Models.Node

  describe "Node.new/1" do
    test "creates a valid node with required fields" do
      assert {:ok, node} = Node.new(hostname: "test-node", address: "192.168.1.1")
      assert node.hostname == "test-node"
      assert node.address == "192.168.1.1"
      assert node.port == 50051
      assert node.status == :offline
      assert node.id =~ ~r/^node-/
      assert %DateTime{} = node.registered_at
    end

    test "accepts all valid fields" do
      attrs = [
        id: "node-123",
        hostname: "test-node",
        address: "192.168.1.1",
        port: 8080,
        status: :online,
        capabilities: ["wasm", "gpu"],
        resources: %{cpu: 4, memory: 8192},
        fault_domain: "rack-1",
        architecture: "arm64"
      ]

      assert {:ok, node} = Node.new(attrs)
      assert node.id == "node-123"
      assert node.port == 8080
      assert node.status == :online
      assert node.capabilities == ["wasm", "gpu"]
      assert node.architecture == "arm64"
    end

    test "returns error for invalid port" do
      assert {:error, errors} = Node.new(hostname: "test", address: "1.2.3.4", port: 0)
      assert "port must be between 1 and 65535" in errors

      assert {:error, errors} = Node.new(hostname: "test", address: "1.2.3.4", port: 100_000)
      assert "port must be between 1 and 65535" in errors
    end

    test "returns error for missing required fields" do
      assert {:error, errors} = Node.new(address: "1.2.3.4")
      assert "hostname is required" in errors

      assert {:error, errors} = Node.new(hostname: "test")
      assert "address is required" in errors
    end
  end

  describe "Node.heartbeat/1" do
    test "updates status to online and sets timestamp" do
      {:ok, node} = Node.new(hostname: "test", address: "1.2.3.4")
      node = Node.heartbeat(node)

      assert node.status == :online
      assert %DateTime{} = node.last_heartbeat
    end
  end

  describe "Node.healthy?/1" do
    test "returns false for nil heartbeat" do
      {:ok, node} = Node.new(hostname: "test", address: "1.2.3.4")
      refute Node.healthy?(node)
    end

    test "returns false for offline status" do
      {:ok, node} = Node.new(hostname: "test", address: "1.2.3.4")
      node = %{node | status: :offline, last_heartbeat: DateTime.utc_now()}
      refute Node.healthy?(node)
    end

    test "returns true for recent heartbeat with online status" do
      {:ok, node} = Node.new(hostname: "test", address: "1.2.3.4")
      node = Node.heartbeat(node)
      assert Node.healthy?(node)
    end
  end

  describe "Node.has_capabilities?/2" do
    test "returns true when all capabilities present" do
      {:ok, node} =
        Node.new(hostname: "test", address: "1.2.3.4", capabilities: ["wasm", "gpu", "fpga"])

      assert Node.has_capabilities?(node, ["wasm", "gpu"])
      assert Node.has_capabilities?(node, "wasm")
    end

    test "returns false when capabilities missing" do
      {:ok, node} = Node.new(hostname: "test", address: "1.2.3.4", capabilities: ["wasm"])
      refute Node.has_capabilities?(node, ["wasm", "gpu"])
    end
  end

  describe "JSON encoding" do
    test "encodes to JSON with ISO8601 timestamps" do
      {:ok, node} = Node.new(hostname: "test", address: "1.2.3.4")
      json = Jason.encode!(node)
      assert is_binary(json)
      assert json =~ "\"hostname\":\"test\""
      assert json =~ "registered_at"
    end
  end
end
