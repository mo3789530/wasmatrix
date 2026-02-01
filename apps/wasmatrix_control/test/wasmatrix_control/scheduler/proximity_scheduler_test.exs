defmodule WasmatrixControl.Scheduler.ProximitySchedulerTest do
  use ExUnit.Case
  alias WasmatrixControl.Scheduler.ProximityScheduler
  alias WasmatrixControl.NodeManager
  alias WasmatrixControl.ModuleManager
  alias WasmatrixControl.Models.Node
  alias WasmatrixControl.Models.WasmModule

  setup do
    # Start managers
    {:ok, node_manager} = NodeManager.start_link(name: :test_scheduler_node_manager)
    {:ok, module_manager} = ModuleManager.start_link(name: :test_scheduler_module_manager)

    # Start scheduler with test managers
    {:ok, scheduler} =
      ProximityScheduler.start_link(
        name: :test_scheduler,
        node_manager: node_manager,
        module_manager: module_manager
      )

    {:ok, scheduler: scheduler, node_manager: node_manager, module_manager: module_manager}
  end

  describe "schedule/4" do
    test "successfully schedules a module to capable node", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      # Register a capable node
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "node-1",
          address: "192.168.1.1",
          capabilities: ["wasm", "http"],
          architecture: "x86_64"
        )

      # Send heartbeat to make node healthy
      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      # Upload a module
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "test-module", capabilities: ["wasm"], architecture: ["x86_64"]],
          binary
        )

      # Schedule the module
      assert {:ok, decision} = ProximityScheduler.schedule(scheduler, module.id, %{})
      assert decision.module_id == module.id
      assert decision.node_id == node.id
      assert decision.score > 0.0
      assert decision.priority >= 0
      assert decision.reason != ""
    end

    test "returns error when no capable nodes", %{scheduler: scheduler, module_manager: mm} do
      # Upload a module without registering any nodes
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "orphan-module", capabilities: ["wasm"]],
          binary
        )

      assert {:error, :no_suitable_nodes} = ProximityScheduler.schedule(scheduler, module.id, %{})
    end

    test "respects capability constraints", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      # Register node with limited capabilities
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "limited-node",
          address: "192.168.1.1",
          # Missing "gpu"
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      # Upload module requiring GPU
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "gpu-module", capabilities: ["wasm", "gpu"]],
          binary
        )

      # Should fail because node doesn't have GPU
      assert {:error, :no_suitable_nodes} =
               ProximityScheduler.schedule(scheduler, module.id, %{
                 required_capabilities: ["gpu"]
               })
    end

    test "respects architecture constraints", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      # Register ARM node
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "arm-node",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          architecture: "arm64"
        )

      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      # Upload x86-only module
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "x86-module", capabilities: ["wasm"], architecture: ["x86_64"]],
          binary
        )

      # Should fail because architecture doesn't match
      assert {:error, :no_suitable_nodes} = ProximityScheduler.schedule(scheduler, module.id, %{})
    end

    test "excludes specified nodes", %{scheduler: scheduler, node_manager: nm, module_manager: mm} do
      # Register two nodes
      {:ok, node1} =
        NodeManager.register_node(nm,
          hostname: "node-1",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      {:ok, node2} =
        NodeManager.register_node(nm,
          hostname: "node-2",
          address: "192.168.1.2",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node1.id)
      NodeManager.heartbeat(nm, node2.id)
      Process.sleep(10)

      # Upload module
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "test-module", capabilities: ["wasm"]],
          binary
        )

      # Schedule with exclusion
      assert {:ok, decision} =
               ProximityScheduler.schedule(scheduler, module.id, %{
                 excluded_nodes: [node1.id]
               })

      assert decision.node_id == node2.id
    end

    test "decision includes scoring factors", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "test-node",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          resources: %{cpu: 8, memory: 16384},
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "test-module", capabilities: ["wasm"]],
          binary
        )

      assert {:ok, decision} = ProximityScheduler.schedule(scheduler, module.id, %{})
      assert decision.factors.proximity_score >= 0.0
      assert decision.factors.resource_score >= 0.0
      assert decision.factors.architecture == "x86_64"
      assert is_map(decision.factors.weights_applied)
    end

    test "schedules within 5ms target", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      # Register node
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "fast-node",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "fast-module", capabilities: ["wasm"]],
          binary
        )

      # Measure scheduling time
      start = System.monotonic_time(:microsecond)
      {:ok, _decision} = ProximityScheduler.schedule(scheduler, module.id, %{})
      elapsed_us = System.monotonic_time(:microsecond) - start
      elapsed_ms = elapsed_us / 1000

      # Should be under 5ms
      assert elapsed_ms < 5.0, "Scheduling took #{elapsed_ms}ms, expected < 5ms"
    end
  end

  describe "schedule_batch/3" do
    test "schedules multiple modules efficiently", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      # Register multiple nodes
      {:ok, node1} =
        NodeManager.register_node(nm,
          hostname: "node-1",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      {:ok, node2} =
        NodeManager.register_node(nm,
          hostname: "node-2",
          address: "192.168.1.2",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node1.id)
      NodeManager.heartbeat(nm, node2.id)
      Process.sleep(10)

      # Upload multiple modules
      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module1} =
        ModuleManager.upload_module(
          mm,
          [name: "module-1", capabilities: ["wasm"]],
          binary
        )

      {:ok, module2} =
        ModuleManager.upload_module(
          mm,
          [name: "module-2", capabilities: ["wasm"]],
          binary
        )

      # Batch schedule
      assert {:ok, decisions} =
               ProximityScheduler.schedule_batch(scheduler, [module1.id, module2.id], %{})

      assert length(decisions) == 2

      # Verify each module got scheduled
      module_ids = Enum.map(decisions, & &1.module_id)
      assert module1.id in module_ids
      assert module2.id in module_ids
    end
  end

  describe "caching" do
    test "caches scheduling decisions", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "cache-node",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "cache-module", capabilities: ["wasm"]],
          binary
        )

      # First call - cache miss
      {:ok, decision1} = ProximityScheduler.schedule(scheduler, module.id, %{})

      # Second call - should hit cache
      {:ok, decision2} = ProximityScheduler.schedule(scheduler, module.id, %{})

      # Same decision
      assert decision1.id == decision2.id

      # Check stats
      {:ok, stats} = ProximityScheduler.get_stats(scheduler)
      assert stats.cache_hits >= 1
      assert stats.cache_misses >= 1
    end
  end

  describe "statistics" do
    test "tracks scheduling statistics", %{
      scheduler: scheduler,
      node_manager: nm,
      module_manager: mm
    } do
      # Initially zero
      {:ok, stats} = ProximityScheduler.get_stats(scheduler)
      assert stats.total_scheduled == 0

      # Register and schedule
      {:ok, node} =
        NodeManager.register_node(nm,
          hostname: "stats-node",
          address: "192.168.1.1",
          capabilities: ["wasm"],
          architecture: "x86_64"
        )

      NodeManager.heartbeat(nm, node.id)
      Process.sleep(10)

      binary = <<0x00, 0x61, 0x73, 0x6D>>

      {:ok, module} =
        ModuleManager.upload_module(
          mm,
          [name: "stats-module", capabilities: ["wasm"]],
          binary
        )

      ProximityScheduler.schedule(scheduler, module.id, %{})

      # Stats updated
      {:ok, stats} = ProximityScheduler.get_stats(scheduler)
      assert stats.total_scheduled == 1
      assert stats.avg_decision_time_ms > 0
    end
  end

  describe "weight customization" do
    test "allows custom scoring weights", %{scheduler: scheduler} do
      # Update all weights explicitly to avoid normalization surprises
      assert {:ok, new_weights} =
               ProximityScheduler.update_weights(scheduler, %{
                 proximity: 0.5,
                 resources: 0.3,
                 fault_domain: 0.1,
                 architecture: 0.05,
                 capabilities: 0.05
               })

      # Weights should be normalized to sum to 1.0
      total = Enum.sum(Map.values(new_weights))
      assert abs(total - 1.0) < 0.01

      # Proximity should have highest weight
      assert new_weights.proximity > new_weights.resources
      assert new_weights.resources > new_weights.fault_domain
    end
  end
end
