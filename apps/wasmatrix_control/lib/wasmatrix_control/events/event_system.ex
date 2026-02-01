defmodule WasmatrixControl.Events.EventSystem do
  @moduledoc """
  GenServer for event-driven communication and module lifecycle triggers.

  Provides:
  - Event publishing with guaranteed delivery
  - Subscription management for event patterns
  - Event-driven module lifecycle triggers
  - Retry mechanisms with exponential backoff
  - Backpressure control for event storms

  Transport agnostic - supports local (test), MQTT, or NATS backends.

  Requirements: 9.1, 9.2, 9.3, 9.4, 9.5
  """

  use GenServer

  alias WasmatrixControl.Models.EventMessage

  @type subscription :: %{
          id: String.t(),
          pattern: String.t() | Regex.t(),
          pid: pid(),
          ref: reference(),
          created_at: DateTime.t()
        }

  @type state :: %{
          transport: module() | nil,
          transport_state: term(),
          subscriptions: [subscription()],
          event_buffer: [EventMessage.t()],
          retry_queue: [{EventMessage.t(), non_neg_integer(), DateTime.t()}],
          config: %{
            max_buffer_size: non_neg_integer(),
            retry_attempts: non_neg_integer(),
            base_retry_delay_ms: non_neg_integer(),
            max_retry_delay_ms: non_neg_integer(),
            backpressure_threshold: non_neg_integer()
          },
          stats: %{
            events_published: non_neg_integer(),
            events_delivered: non_neg_integer(),
            events_failed: non_neg_integer(),
            retries_attempted: non_neg_integer()
          }
        }

  @default_config %{
    max_buffer_size: 1000,
    retry_attempts: 3,
    base_retry_delay_ms: 100,
    max_retry_delay_ms: 5000,
    backpressure_threshold: 100
  }

  # Client API

  def start_link(opts \\ []) do
    name = opts[:name] || __MODULE__
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  @doc """
  Publishes an event to the system.

  Returns :ok on success, {:error, reason} on failure.
  """
  def publish(server \\ __MODULE__, %EventMessage{} = event) do
    GenServer.call(server, {:publish, event}, 5_000)
  end

  @doc """
  Creates and publishes an event.
  """
  def publish_event(server \\ __MODULE__, attrs) do
    case EventMessage.new(attrs) do
      {:ok, event} -> publish(server, event)
      error -> error
    end
  end

  @doc """
  Subscribes to events matching a pattern.

  Pattern can be:
  - "exact.match" - exact string match
  - "prefix.*" - wildcard suffix
  - ~r/regex/ - regex pattern

  Returns {:ok, subscription_id} on success.
  """
  def subscribe(server \\ __MODULE__, pattern, pid \\ self()) do
    GenServer.call(server, {:subscribe, pattern, pid}, 5_000)
  end

  @doc """
  Unsubscribes from events.
  """
  def unsubscribe(server \\ __MODULE__, subscription_id) do
    GenServer.call(server, {:unsubscribe, subscription_id}, 5_000)
  end

  @doc """
  Lists all active subscriptions.
  """
  def list_subscriptions(server \\ __MODULE__) do
    GenServer.call(server, :list_subscriptions)
  end

  @doc """
  Gets event system statistics.
  """
  def get_stats(server \\ __MODULE__) do
    GenServer.call(server, :get_stats)
  end

  @doc """
  Manually triggers retry processing.
  """
  def process_retries(server \\ __MODULE__) do
    GenServer.cast(server, :process_retries)
  end

  @doc """
  Configures backpressure settings.
  """
  def configure(server \\ __MODULE__, config) do
    GenServer.call(server, {:configure, config})
  end

  # Server Callbacks

  @impl true
  def init(opts) do
    transport = opts[:transport] || WasmatrixControl.Events.LocalTransport
    config = Map.merge(@default_config, opts[:config] || %{})

    # Initialize transport
    {:ok, transport_state} = transport.init(opts[:transport_opts] || [])

    state = %{
      transport: transport,
      transport_state: transport_state,
      subscriptions: [],
      event_buffer: [],
      retry_queue: [],
      config: config,
      stats: %{
        events_published: 0,
        events_delivered: 0,
        events_failed: 0,
        retries_attempted: 0
      }
    }

    # Schedule retry processing
    schedule_retry_processing(config.base_retry_delay_ms)

    {:ok, state}
  end

  @impl true
  def handle_call({:publish, %EventMessage{} = event}, _from, state) do
    # Check if we're at capacity
    current_load = length(state.event_buffer) + length(state.retry_queue)

    if current_load >= state.config.backpressure_threshold do
      # Over capacity - reject with backpressure
      {:reply, {:error, :backpressure_active}, state}
    else
      # Under capacity - add to buffer and process
      new_buffer = [event | state.event_buffer]

      case deliver_event(event, state) do
        {:ok, delivered_state} ->
          new_state = update_in(delivered_state, [:stats, :events_published], &(&1 + 1))
          # Remove from buffer after successful delivery
          new_state = %{new_state | event_buffer: List.delete(new_buffer, event)}
          {:reply, :ok, new_state}

        {:error, _reason} ->
          retry_entry = {event, 0, DateTime.utc_now()}

          new_state = %{
            state
            | event_buffer: new_buffer,
              retry_queue: [retry_entry | state.retry_queue],
              stats: %{state.stats | events_published: state.stats.events_published + 1}
          }

          {:reply, {:error, :queued_for_retry}, new_state}
      end
    end
  end

  @impl true
  def handle_call({:subscribe, pattern, pid}, _from, state) do
    subscription_id = generate_subscription_id()
    ref = Process.monitor(pid)

    subscription = %{
      id: subscription_id,
      pattern: normalize_pattern(pattern),
      pid: pid,
      ref: ref,
      created_at: DateTime.utc_now()
    }

    new_state = %{state | subscriptions: [subscription | state.subscriptions]}

    {:reply, {:ok, subscription_id}, new_state}
  end

  @impl true
  def handle_call({:unsubscribe, subscription_id}, _from, state) do
    case Enum.find(state.subscriptions, &(&1.id == subscription_id)) do
      nil ->
        {:reply, {:error, :not_found}, state}

      subscription ->
        Process.demonitor(subscription.ref, [:flush])

        new_state = %{
          state
          | subscriptions: Enum.reject(state.subscriptions, &(&1.id == subscription_id))
        }

        {:reply, :ok, new_state}
    end
  end

  @impl true
  def handle_call(:list_subscriptions, _from, state) do
    subs =
      Enum.map(state.subscriptions, fn sub ->
        %{
          id: sub.id,
          pattern: sub.pattern,
          pid: sub.pid,
          created_at: sub.created_at
        }
      end)

    {:reply, {:ok, subs}, state}
  end

  @impl true
  def handle_call(:get_stats, _from, state) do
    stats = Map.put(state.stats, :buffer_size, length(state.event_buffer))
    stats = Map.put(stats, :retry_queue_size, length(state.retry_queue))
    stats = Map.put(stats, :subscription_count, length(state.subscriptions))

    {:reply, {:ok, stats}, state}
  end

  @impl true
  def handle_call({:configure, new_config}, _from, state) do
    merged_config = Map.merge(state.config, new_config)
    {:reply, {:ok, merged_config}, %{state | config: merged_config}}
  end

  @impl true
  def handle_cast(:process_retries, state) do
    {retry_success, retry_failed, new_queue, new_stats} =
      process_retry_queue(state.retry_queue, state)

    new_state = %{state | retry_queue: new_queue, stats: new_stats}

    # Schedule next retry processing
    schedule_retry_processing(state.config.base_retry_delay_ms)

    {:noreply, new_state}
  end

  @impl true
  def handle_info({:DOWN, ref, :process, pid, _reason}, state) do
    # Remove subscriptions for dead processes
    new_subscriptions =
      Enum.reject(state.subscriptions, fn sub ->
        sub.ref == ref and sub.pid == pid
      end)

    {:noreply, %{state | subscriptions: new_subscriptions}}
  end

  @impl true
  def handle_info(:process_retries, state) do
    handle_cast(:process_retries, state)
  end

  # Private Functions

  defp deliver_event(%EventMessage{} = event, state) do
    # First, deliver to local subscribers
    matches = find_matching_subscriptions(event.type, state.subscriptions)

    delivered =
      Enum.reduce(matches, 0, fn sub, count ->
        send(sub.pid, {:event, event})
        count + 1
      end)

    # Then, attempt transport delivery
    case state.transport do
      nil ->
        {:ok,
         %{
           state
           | stats: %{state.stats | events_delivered: state.stats.events_delivered + delivered}
         }}

      transport ->
        case transport.publish(event, state.transport_state) do
          {:ok, new_transport_state} ->
            {:ok,
             %{
               state
               | transport_state: new_transport_state,
                 stats: %{
                   state.stats
                   | events_delivered: state.stats.events_delivered + delivered + 1
                 }
             }}

          {:error, reason} ->
            {:error, reason,
             %{state | stats: %{state.stats | events_failed: state.stats.events_failed + 1}}}
        end
    end
  end

  defp process_retry_queue(queue, state) do
    now = DateTime.utc_now()
    config = state.config

    Enum.reduce(queue, {0, 0, [], state.stats}, fn {event, attempts, _queued_at},
                                                   {success, failed, new_queue, stats} ->
      if attempts >= config.retry_attempts do
        # Max retries reached
        {success, failed + 1, new_queue, %{stats | events_failed: stats.events_failed + 1}}
      else
        # Calculate delay with exponential backoff
        delay =
          min(
            config.base_retry_delay_ms * :math.pow(2, attempts),
            config.max_retry_delay_ms
          )
          |> round()

        # Check if enough time has passed
        time_elapsed = DateTime.diff(now, _queued_at, :millisecond)

        if time_elapsed >= delay do
          # Attempt retry
          case deliver_event(event, %{state | stats: stats}) do
            {:ok, _} ->
              {success + 1, failed, new_queue,
               %{stats | retries_attempted: stats.retries_attempted + 1}}

            {:error, _, _} ->
              # Re-queue with incremented attempt count
              {success, failed, [{event, attempts + 1, now} | new_queue],
               %{stats | retries_attempted: stats.retries_attempted + 1}}
          end
        else
          # Not ready for retry yet
          {success, failed, [{event, attempts, _queued_at} | new_queue], stats}
        end
      end
    end)
  end

  defp find_matching_subscriptions(event_type, subscriptions) do
    Enum.filter(subscriptions, fn sub ->
      matches_pattern?(event_type, sub.pattern)
    end)
  end

  defp matches_pattern?(event_type, pattern) when is_binary(pattern) do
    # Handle wildcard patterns
    if String.ends_with?(pattern, "*") do
      prefix = String.slice(pattern, 0..-2//1)
      String.starts_with?(event_type, prefix)
    else
      event_type == pattern
    end
  end

  defp matches_pattern?(event_type, %Regex{} = pattern) do
    Regex.match?(pattern, event_type)
  end

  defp normalize_pattern(pattern) when is_binary(pattern), do: pattern
  defp normalize_pattern(%Regex{} = pattern), do: pattern
  defp normalize_pattern(pattern), do: to_string(pattern)

  defp generate_subscription_id do
    "sub-" <> Base.encode16(:crypto.strong_rand_bytes(8), case: :lower)
  end

  defp schedule_retry_processing(delay_ms) do
    Process.send_after(self(), :process_retries, delay_ms)
  end
end
