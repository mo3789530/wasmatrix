defmodule WasmatrixControl.State.StateManagerTest do
  use ExUnit.Case
  alias WasmatrixControl.State.StateManager
  alias WasmatrixControl.State.MemoryBackend

  setup do
    {:ok, pid} =
      StateManager.start_link(
        name: nil,
        backend: MemoryBackend
      )

    {:ok, state_manager: pid}
  end

  describe "get/2" do
    test "returns nil for non-existent key", %{state_manager: sm} do
      assert {:ok, nil} = StateManager.get(sm, "non-existent-key")
    end

    test "returns value after put", %{state_manager: sm} do
      :ok = StateManager.put(sm, "test-key", "test-value")
      assert {:ok, "test-value"} = StateManager.get(sm, "test-key")
    end
  end

  describe "put/3" do
    test "stores value", %{state_manager: sm} do
      assert :ok = StateManager.put(sm, "key1", %{data: "value"})
      assert {:ok, %{data: "value"}} = StateManager.get(sm, "key1")
    end

    test "overwrites existing value", %{state_manager: sm} do
      :ok = StateManager.put(sm, "key2", "original")
      :ok = StateManager.put(sm, "key2", "updated")
      assert {:ok, "updated"} = StateManager.get(sm, "key2")
    end

    test "stores different data types", %{state_manager: sm} do
      # String
      :ok = StateManager.put(sm, "str", "hello")
      assert {:ok, "hello"} = StateManager.get(sm, "str")

      # Integer
      :ok = StateManager.put(sm, "int", 42)
      assert {:ok, 42} = StateManager.get(sm, "int")

      # Map
      :ok = StateManager.put(sm, "map", %{a: 1, b: 2})
      assert {:ok, %{a: 1, b: 2}} = StateManager.get(sm, "map")

      # List
      :ok = StateManager.put(sm, "list", [1, 2, 3])
      assert {:ok, [1, 2, 3]} = StateManager.get(sm, "list")
    end
  end

  describe "delete/2" do
    test "removes key", %{state_manager: sm} do
      :ok = StateManager.put(sm, "delete-me", "value")
      :ok = StateManager.delete(sm, "delete-me")
      assert {:ok, nil} = StateManager.get(sm, "delete-me")
    end

    test "succeeds for non-existent key", %{state_manager: sm} do
      assert :ok = StateManager.delete(sm, "never-existed")
    end
  end

  describe "cas/4" do
    test "updates when current value matches", %{state_manager: sm} do
      :ok = StateManager.put(sm, "cas-key", "original")
      assert :ok = StateManager.cas(sm, "cas-key", "original", "new-value")
      assert {:ok, "new-value"} = StateManager.get(sm, "cas-key")
    end

    test "fails when current value doesn't match", %{state_manager: sm} do
      :ok = StateManager.put(sm, "cas-key2", "current")
      assert {:error, :cas_failed} = StateManager.cas(sm, "cas-key2", "wrong", "new")
      assert {:ok, "current"} = StateManager.get(sm, "cas-key2")
    end
  end

  describe "keys/2" do
    test "returns all keys", %{state_manager: sm} do
      :ok = StateManager.put(sm, "a", 1)
      :ok = StateManager.put(sm, "b", 2)
      :ok = StateManager.put(sm, "c", 3)

      {:ok, keys} = StateManager.keys(sm)
      assert length(keys) == 3
      assert "a" in keys
      assert "b" in keys
      assert "c" in keys
    end

    test "filters by prefix", %{state_manager: sm} do
      :ok = StateManager.put(sm, "user:1", %{name: "Alice"})
      :ok = StateManager.put(sm, "user:2", %{name: "Bob"})
      :ok = StateManager.put(sm, "config:app", %{debug: true})

      {:ok, keys} = StateManager.keys(sm, "user:")
      assert length(keys) == 2
      assert "user:1" in keys
      assert "user:2" in keys
      refute "config:app" in keys
    end
  end

  describe "statistics" do
    test "tracks reads and writes", %{state_manager: sm} do
      # Perform some operations
      StateManager.put(sm, "stat-key", "value")
      StateManager.get(sm, "stat-key")
      StateManager.get(sm, "stat-key")

      {:ok, stats} = StateManager.get_stats(sm)
      assert stats.writes == 1
      assert stats.reads == 2
    end

    test "tracks cache performance", %{state_manager: sm} do
      # First read - cache miss
      StateManager.put(sm, "cache-key", "value")
      StateManager.get(sm, "cache-key")

      # Second read - cache hit
      StateManager.get(sm, "cache-key")

      {:ok, stats} = StateManager.get_stats(sm)
      assert stats.cache_misses == 1
      assert stats.cache_hits == 1
    end
  end

  describe "clear_cache/1" do
    test "clears local cache", %{state_manager: sm} do
      :ok = StateManager.put(sm, "clear-me", "value")
      # Load into cache
      {:ok, _} = StateManager.get(sm, "clear-me")

      # Clear cache
      :ok = StateManager.clear_cache(sm)

      # Value should still be available from CRDT
      assert {:ok, "value"} = StateManager.get(sm, "clear-me")
    end
  end

  describe "change subscription" do
    test "receives change notifications", %{state_manager: sm} do
      # Subscribe to changes
      {:ok, ref} = StateManager.subscribe_changes(sm, "notify:")

      # Put a value matching the pattern
      :ok = StateManager.put(sm, "notify:test", "value1")

      # Should receive notification
      assert_receive {:state_change, "notify:test", "value1"}, 100

      # Clean up subscription
      Process.demonitor(ref, [:flush])
    end

    test "doesn't receive notifications for non-matching keys", %{state_manager: sm} do
      # Subscribe to specific prefix
      {:ok, ref} = StateManager.subscribe_changes(sm, "specific:")

      # Put a value not matching the pattern
      :ok = StateManager.put(sm, "other:key", "value")

      # Should not receive notification
      refute_receive {:state_change, _, _}, 100

      Process.demonitor(ref, [:flush])
    end
  end

  describe "integration with backend" do
    test "persists to backend", %{state_manager: sm} do
      # Store value
      :ok = StateManager.put(sm, "persisted", "data")

      # Clear cache
      :ok = StateManager.clear_cache(sm)

      # Value should still be available (from CRDT/backend)
      assert {:ok, "data"} = StateManager.get(sm, "persisted")
    end
  end
end
