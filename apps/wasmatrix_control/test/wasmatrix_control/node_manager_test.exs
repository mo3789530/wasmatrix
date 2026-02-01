defmodule WasmatrixControl.NodeManagerTest do
  use ExUnit.Case
  alias WasmatrixControl.NodeManager
  alias WasmatrixControl.Models.Node

  setup do
    # Start a fresh NodeManager for each test
    {:ok, pid} = NodeManager.start_link(name: :test_node_manager)
    {:ok, manager: pid}
  end

  describe "register_node/2" do
    test "registers a new node successfully", %{manager: manager} do
      attrs = [
        hostname: "test-node-1",
        address: "192.168.1.1",
        capabilities: ["wasm", "gpu"],
        resources: %{cpu: 4, memory: 8192}
      ]

      assert {:ok, node} = NodeManager.register_node(manager, attrs)
      assert node.hostname == "test-node-1"
      assert node.status == :offline
      assert "wasm" in node.capabilities
    end

    test "returns error for invalid node", %{manager: manager} do
      assert {:error, errors} = NodeManager.register_node(manager, [])
      assert is_list(errors)
    end
  end

  describe "heartbeat/2" do
    test "updates node heartbeat and status", %{manager: manager} do
      {:ok, node} =
        NodeManager.register_node(manager,
          hostname: "test",
          address: "1.2.3.4"
        )

      :ok = NodeManager.heartbeat(manager, node.id)

      # Give the cast time to process
      Process.sleep(10)

      {:ok, updated} = NodeManager.get_node(manager, node.id)
      assert updated.status == :online
      assert updated.last_heartbeat != nil
    end
  end

  describe "get_node/2" do
    test "returns node by id", %{manager: manager} do
      {:ok, node} =
        NodeManager.register_node(manager,
          hostname: "test",
          address: "1.2.3.4"
        )

      assert {:ok, ^node} = NodeManager.get_node(manager, node.id)
    end

    test "returns error for unknown node", %{manager: manager} do
      assert {:error, :not_found} = NodeManager.get_node(manager, "unknown-id")
    end
  end

  describe "list_nodes/1" do
    test "returns all registered nodes", %{manager: manager} do
      {:ok, node1} =
        NodeManager.register_node(manager,
          hostname: "node1",
          address: "1.2.3.4"
        )

      {:ok, node2} =
        NodeManager.register_node(manager,
          hostname: "node2",
          address: "5.6.7.8"
        )

      {:ok, nodes} = NodeManager.list_nodes(manager)
      assert length(nodes) == 2
      assert node1.id in Enum.map(nodes, & &1.id)
      assert node2.id in Enum.map(nodes, & &1.id)
    end

    test "returns empty list when no nodes", %{manager: manager} do
      {:ok, nodes} = NodeManager.list_nodes(manager)
      assert nodes == []
    end
  end

  describe "list_nodes_by_status/2" do
    test "returns nodes filtered by status", %{manager: manager} do
      {:ok, node} =
        NodeManager.register_node(manager,
          hostname: "test",
          address: "1.2.3.4"
        )

      # Initially offline
      {:ok, offline_nodes} = NodeManager.list_nodes_by_status(manager, :offline)
      assert length(offline_nodes) == 1

      # Send heartbeat to make online
      :ok = NodeManager.heartbeat(manager, node.id)
      Process.sleep(10)

      {:ok, online_nodes} = NodeManager.list_nodes_by_status(manager, :online)
      assert length(online_nodes) == 1

      {:ok, offline_nodes} = NodeManager.list_nodes_by_status(manager, :offline)
      assert offline_nodes == []
    end
  end

  describe "update_node_metadata/3" do
    test "updates node metadata", %{manager: manager} do
      {:ok, node} =
        NodeManager.register_node(manager,
          hostname: "test",
          address: "1.2.3.4"
        )

      {:ok, updated} =
        NodeManager.update_node_metadata(manager, node.id, %{
          region: "us-east-1",
          rack: "rack-42"
        })

      assert updated.metadata.region == "us-east-1"
      assert updated.metadata.rack == "rack-42"
    end

    test "returns error for unknown node", %{manager: manager} do
      assert {:error, :not_found} = NodeManager.update_node_metadata(manager, "unknown", %{})
    end
  end

  describe "unregister_node/2" do
    test "removes node from registry", %{manager: manager} do
      {:ok, node} =
        NodeManager.register_node(manager,
          hostname: "test",
          address: "1.2.3.4"
        )

      assert :ok = NodeManager.unregister_node(manager, node.id)
      assert {:error, :not_found} = NodeManager.get_node(manager, node.id)
    end

    test "returns error for unknown node", %{manager: manager} do
      assert {:error, :not_found} = NodeManager.unregister_node(manager, "unknown")
    end
  end

  describe "find_capable_nodes/2" do
    test "returns healthy nodes with required capabilities", %{manager: manager} do
      {:ok, node1} =
        NodeManager.register_node(manager,
          hostname: "node1",
          address: "1.2.3.4",
          capabilities: ["wasm", "gpu"]
        )

      {:ok, node2} =
        NodeManager.register_node(manager,
          hostname: "node2",
          address: "5.6.7.8",
          capabilities: ["wasm"]
        )

      # Make both nodes healthy
      :ok = NodeManager.heartbeat(manager, node1.id)
      :ok = NodeManager.heartbeat(manager, node2.id)
      Process.sleep(10)

      {:ok, capable} = NodeManager.find_capable_nodes(manager, ["wasm", "gpu"])
      assert length(capable) == 1
      assert hd(capable).id == node1.id
    end

    test "excludes unhealthy nodes", %{manager: manager} do
      {:ok, node} =
        NodeManager.register_node(manager,
          hostname: "test",
          address: "1.2.3.4",
          capabilities: ["wasm"]
        )

      # Don't send heartbeat - node should be unhealthy
      {:ok, capable} = NodeManager.find_capable_nodes(manager, ["wasm"])
      assert capable == []
    end
  end
end
