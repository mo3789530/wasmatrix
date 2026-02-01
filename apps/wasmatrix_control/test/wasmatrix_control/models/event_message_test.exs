defmodule WasmatrixControl.Models.EventMessageTest do
  use ExUnit.Case
  alias WasmatrixControl.Models.EventMessage

  describe "EventMessage.new/1" do
    test "creates a valid event with required fields" do
      assert {:ok, event} = EventMessage.new(type: "node.registered", source: "test-node")
      assert event.type == "node.registered"
      assert event.source == "test-node"
      assert event.priority == :normal
      assert event.ttl == 60
      assert event.id =~ ~r/^evt-/
      assert event.correlation_id =~ ~r/^evt-/
    end

    test "accepts all valid fields" do
      attrs = [
        type: "module.deployed",
        source: "scheduler",
        target: "node-123",
        payload: %{module_id: "mod-456"},
        metadata: %{user: "admin"},
        priority: :high,
        ttl: 120
      ]

      assert {:ok, event} = EventMessage.new(attrs)
      assert event.target == "node-123"
      assert event.priority == :high
      assert event.ttl == 120
      assert event.payload.module_id == "mod-456"
    end

    test "returns error for invalid type" do
      assert {:error, errors} = EventMessage.new(type: "invalid.type", source: "test")
      assert "invalid event type" in errors
    end

    test "returns error for missing required fields" do
      assert {:error, errors} = EventMessage.new(source: "test")
      assert "type is required" in errors
    end
  end

  describe "EventMessage.expired?/1" do
    test "returns false for fresh event" do
      {:ok, event} = EventMessage.new(type: "node.registered", source: "test")
      refute EventMessage.expired?(event)
    end

    test "returns true for expired event" do
      {:ok, event} = EventMessage.new(type: "node.registered", source: "test", ttl: 0)
      :timer.sleep(10)
      assert EventMessage.expired?(event)
    end
  end

  describe "EventMessage.remaining_ttl/1" do
    test "returns positive ttl for fresh event" do
      {:ok, event} = EventMessage.new(type: "node.registered", source: "test", ttl: 60)
      assert EventMessage.remaining_ttl(event) > 0
    end

    test "returns 0 for expired event" do
      {:ok, event} = EventMessage.new(type: "node.registered", source: "test", ttl: 0)
      :timer.sleep(10)
      assert EventMessage.remaining_ttl(event) == 0
    end
  end

  describe "EventMessage.reply/3" do
    test "creates reply with same correlation_id" do
      {:ok, original} = EventMessage.new(type: "scheduling.request", source: "client")
      {:ok, reply} = EventMessage.reply(original, "scheduling.decision", %{result: "ok"})

      assert reply.type == "scheduling.decision"
      assert reply.correlation_id == original.correlation_id
      assert reply.target == original.source
      assert reply.payload.result == "ok"
    end
  end

  describe "EventMessage.put_metadata/3" do
    test "adds metadata to event" do
      {:ok, event} = EventMessage.new(type: "node.registered", source: "test")
      event = EventMessage.put_metadata(event, "key", "value")
      assert event.metadata["key"] == "value"
    end
  end

  describe "Event builders" do
    test "node_registered/2 creates node registration event" do
      {:ok, event} = EventMessage.node_registered("node-123", %{region: "us-east"})
      assert event.type == "node.registered"
      assert event.source == "node-123"
      assert event.payload.node_id == "node-123"
      assert event.metadata.region == "us-east"
    end

    test "node_heartbeat/2 creates heartbeat event" do
      {:ok, event} = EventMessage.node_heartbeat("node-123", %{cpu: 50})
      assert event.type == "node.heartbeat"
      assert event.priority == :low
      assert event.payload.status.cpu == 50
    end

    test "module_deployed/3 creates deployment event" do
      {:ok, event} = EventMessage.module_deployed("mod-123", "node-456")
      assert event.type == "module.deployed"
      assert event.source == "scheduler"
      assert event.target == "node-456"
    end

    test "scheduling_request/2 creates scheduling request" do
      {:ok, event} = EventMessage.scheduling_request("mod-123", %{priority: :high})
      assert event.type == "scheduling.request"
      assert event.priority == :high
      assert event.payload.constraints.priority == :high
    end
  end

  describe "JSON encoding" do
    test "encodes to JSON" do
      {:ok, event} = EventMessage.new(type: "node.registered", source: "test")
      json = Jason.encode!(event)
      assert is_binary(json)
      assert json =~ "\"type\":\"node.registered\""
    end
  end
end
