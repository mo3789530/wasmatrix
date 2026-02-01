defmodule WasmatrixControl.Events.EventSystemTest do
  use ExUnit.Case
  alias WasmatrixControl.Events.EventSystem
  alias WasmatrixControl.Models.EventMessage

  setup do
    # Use the global EventSystem started by the application
    # Clear any existing state by starting fresh with a unique name
    name = :"test_event_system_#{System.unique_integer([:positive])}"
    {:ok, pid} = EventSystem.start_link(name: name)
    {:ok, event_system: pid}
  end

  describe "publish/2" do
    test "successfully publishes an event", %{event_system: es} do
      {:ok, event} =
        EventMessage.new(
          type: "test.event",
          source: "test",
          payload: %{data: "hello"}
        )

      assert :ok = EventSystem.publish(es, event)

      # Check stats
      {:ok, stats} = EventSystem.get_stats(es)
      assert stats.events_published == 1
    end

    test "returns error on backpressure", %{event_system: es} do
      # Set low threshold to trigger backpressure
      EventSystem.configure(es, %{backpressure_threshold: 5, max_buffer_size: 10})

      # Publish many events quickly
      results =
        for i <- 1..10 do
          {:ok, event} =
            EventMessage.new(
              type: "flood.event",
              source: "test",
              payload: %{index: i}
            )

          EventSystem.publish(es, event)
        end

      # Some should succeed, some should fail with backpressure
      assert :ok in results
      assert {:error, :backpressure_active} in results
    end

    test "publishes event with publish_event helper", %{event_system: es} do
      assert :ok =
               EventSystem.publish_event(es,
                 type: "node.registered",
                 source: "node-123"
               )

      {:ok, stats} = EventSystem.get_stats(es)
      assert stats.events_published == 1
    end
  end

  describe "subscribe/3" do
    test "creates a subscription", %{event_system: es} do
      assert {:ok, sub_id} = EventSystem.subscribe(es, "test.event")
      assert is_binary(sub_id)
      assert String.starts_with?(sub_id, "sub-")

      {:ok, subs} = EventSystem.list_subscriptions(es)
      assert length(subs) == 1
      assert hd(subs).id == sub_id
    end

    test "receives events matching subscription pattern", %{event_system: es} do
      # Subscribe to events
      {:ok, _sub_id} = EventSystem.subscribe(es, "user.created")

      # Publish matching event
      {:ok, event} =
        EventMessage.new(
          type: "user.created",
          source: "api",
          payload: %{user_id: "123"}
        )

      EventSystem.publish(es, event)

      # Should receive the event
      assert_receive {:event, received_event}, 100
      assert received_event.type == "user.created"
      assert received_event.payload.user_id == "123"
    end

    test "doesn't receive non-matching events", %{event_system: es} do
      # Subscribe to specific pattern
      {:ok, _sub_id} = EventSystem.subscribe(es, "user.created")

      # Publish different event
      {:ok, event} =
        EventMessage.new(
          type: "user.deleted",
          source: "api"
        )

      EventSystem.publish(es, event)

      # Should not receive
      refute_receive {:event, _}, 100
    end

    test "wildcard subscription receives matching events", %{event_system: es} do
      # Subscribe with wildcard
      {:ok, _sub_id} = EventSystem.subscribe(es, "user.*")

      # Publish various events
      for type <- ["user.created", "user.updated", "user.deleted"] do
        {:ok, event} = EventMessage.new(type: type, source: "api")
        EventSystem.publish(es, event)
      end

      # Should receive all 3 events
      assert_receive {:event, _}, 100
      assert_receive {:event, _}, 100
      assert_receive {:event, _}, 100
    end

    test "regex subscription receives matching events", %{event_system: es} do
      # Subscribe with regex
      {:ok, _sub_id} = EventSystem.subscribe(es, ~r/^user\.(created|updated)$/)

      # Publish events
      {:ok, created} = EventMessage.new(type: "user.created", source: "api")
      {:ok, deleted} = EventMessage.new(type: "user.deleted", source: "api")

      EventSystem.publish(es, created)
      EventSystem.publish(es, deleted)

      # Should only receive created
      assert_receive {:event, received}, 100
      assert received.type == "user.created"
      refute_receive {:event, _}, 100
    end

    test "cleans up subscription on process exit", %{event_system: es} do
      # Create a temporary process that subscribes
      subscriber =
        spawn(fn ->
          {:ok, _sub_id} = EventSystem.subscribe(es, "test.event")

          receive do
            :die -> :ok
          end
        end)

      Process.sleep(50)

      # Verify subscription exists
      {:ok, subs} = EventSystem.list_subscriptions(es)
      assert length(subs) == 1

      # Kill the subscriber
      send(subscriber, :die)
      Process.sleep(50)

      # Subscription should be cleaned up
      {:ok, subs} = EventSystem.list_subscriptions(es)
      assert subs == []
    end
  end

  describe "unsubscribe/2" do
    test "removes subscription", %{event_system: es} do
      {:ok, sub_id} = EventSystem.subscribe(es, "test.event")
      assert :ok = EventSystem.unsubscribe(es, sub_id)

      {:ok, subs} = EventSystem.list_subscriptions(es)
      assert subs == []
    end

    test "returns error for unknown subscription", %{event_system: es} do
      assert {:error, :not_found} = EventSystem.unsubscribe(es, "unknown-id")
    end

    test "unsubscribed process no longer receives events", %{event_system: es} do
      {:ok, sub_id} = EventSystem.subscribe(es, "test.event")
      assert :ok = EventSystem.unsubscribe(es, sub_id)

      # Publish event
      {:ok, event} = EventMessage.new(type: "test.event", source: "test")
      EventSystem.publish(es, event)

      # Should not receive
      refute_receive {:event, _}, 100
    end
  end

  describe "list_subscriptions/1" do
    test "returns empty list when no subscriptions", %{event_system: es} do
      {:ok, subs} = EventSystem.list_subscriptions(es)
      assert subs == []
    end

    test "returns all subscriptions", %{event_system: es} do
      {:ok, sub1} = EventSystem.subscribe(es, "event.a")
      {:ok, sub2} = EventSystem.subscribe(es, "event.b")

      {:ok, subs} = EventSystem.list_subscriptions(es)
      assert length(subs) == 2
      ids = Enum.map(subs, & &1.id)
      assert sub1 in ids
      assert sub2 in ids
    end
  end

  describe "statistics" do
    test "tracks event statistics", %{event_system: es} do
      # Publish some events
      for i <- 1..5 do
        {:ok, event} = EventMessage.new(type: "stat.event", source: "test", payload: %{i: i})
        EventSystem.publish(es, event)
      end

      {:ok, stats} = EventSystem.get_stats(es)
      assert stats.events_published == 5
      assert stats.events_delivered >= 0
      assert stats.subscription_count == 0
      assert stats.buffer_size == 0
      assert stats.retry_queue_size == 0
    end
  end

  describe "configuration" do
    test "allows configuration updates", %{event_system: es} do
      {:ok, config} = EventSystem.configure(es, %{retry_attempts: 5, base_retry_delay_ms: 200})

      assert config.retry_attempts == 5
      assert config.base_retry_delay_ms == 200
    end
  end

  describe "event message builders" do
    test "node_registered creates proper event", %{event_system: es} do
      {:ok, event} = EventMessage.node_registered("node-123", %{region: "us-east"})

      assert event.type == "node.registered"
      assert event.source == "node-123"
      assert event.payload.node_id == "node-123"
      assert event.metadata.region == "us-east"

      assert :ok = EventSystem.publish(es, event)
    end

    test "scheduling_request creates proper event", %{event_system: es} do
      {:ok, event} = EventMessage.scheduling_request("mod-123", %{priority: :high})

      assert event.type == "scheduling.request"
      assert event.source == "api"
      assert event.priority == :high
      assert event.payload.constraints.priority == :high

      assert :ok = EventSystem.publish(es, event)
    end

    test "reply creates correlated event", %{event_system: es} do
      {:ok, original} =
        EventMessage.new(
          type: "request",
          source: "client",
          payload: %{}
        )

      {:ok, reply} = EventMessage.reply(original, "response", %{result: "ok"})

      assert reply.type == "response"
      assert reply.correlation_id == original.correlation_id
      assert reply.target == original.source
    end
  end
end
