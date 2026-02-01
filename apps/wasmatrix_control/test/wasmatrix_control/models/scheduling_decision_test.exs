defmodule WasmatrixControl.Models.SchedulingDecisionTest do
  use ExUnit.Case
  alias WasmatrixControl.Models.SchedulingDecision

  describe "SchedulingDecision.new/1" do
    test "creates a valid decision with required fields" do
      assert {:ok, decision} = SchedulingDecision.new(module_id: "mod-123", node_id: "node-456")
      assert decision.module_id == "mod-123"
      assert decision.node_id == "node-456"
      assert decision.status == :pending
      assert decision.priority == 0
      assert decision.score == 0.0
      assert decision.id =~ ~r/^sched-/
    end

    test "accepts all valid fields" do
      attrs = [
        module_id: "mod-123",
        node_id: "node-456",
        priority: 10,
        score: 0.95,
        reason: "Optimal placement",
        factors: %{proximity: 0.9, resources: 0.8},
        execution_policy: :queued
      ]

      assert {:ok, decision} = SchedulingDecision.new(attrs)
      assert decision.priority == 10
      assert decision.score == 0.95
      assert decision.execution_policy == :queued
    end

    test "returns error for invalid score" do
      assert {:error, errors} =
               SchedulingDecision.new(module_id: "mod", node_id: "node", score: 1.5)

      assert "score must be between 0.0 and 1.0" in errors

      assert {:error, errors} =
               SchedulingDecision.new(module_id: "mod", node_id: "node", score: -0.1)

      assert "score must be between 0.0 and 1.0" in errors
    end

    test "returns error for missing required fields" do
      assert {:error, errors} = SchedulingDecision.new(node_id: "node")
      assert "module_id is required" in errors
    end
  end

  describe "SchedulingDecision.accept/1" do
    test "marks decision as accepted" do
      {:ok, decision} = SchedulingDecision.new(module_id: "mod", node_id: "node")
      decision = SchedulingDecision.accept(decision)
      assert decision.status == :accepted
    end
  end

  describe "SchedulingDecision.reject/2" do
    test "marks decision as rejected with reason" do
      {:ok, decision} = SchedulingDecision.new(module_id: "mod", node_id: "node")
      decision = SchedulingDecision.reject(decision, "Node overloaded")
      assert decision.status == :rejected
      assert decision.reason == "Node overloaded"
    end
  end

  describe "SchedulingDecision.expired?/1" do
    test "returns false for valid decision" do
      {:ok, decision} = SchedulingDecision.new(module_id: "mod", node_id: "node")
      refute SchedulingDecision.expired?(decision)
    end

    test "returns true for expired decision" do
      {:ok, decision} = SchedulingDecision.new(module_id: "mod", node_id: "node")
      decision = %{decision | valid_until: DateTime.add(DateTime.utc_now(), -1, :second)}
      assert SchedulingDecision.expired?(decision)
    end

    test "returns false for nil valid_until" do
      {:ok, decision} = SchedulingDecision.new(module_id: "mod", node_id: "node")
      decision = %{decision | valid_until: nil}
      refute SchedulingDecision.expired?(decision)
    end
  end

  describe "SchedulingDecision.quality_score/1" do
    test "calculates quality based on score and factors" do
      {:ok, decision} =
        SchedulingDecision.new(
          module_id: "mod",
          node_id: "node",
          score: 0.8,
          factors: %{a: 1, b: 2, c: 3}
        )

      quality = SchedulingDecision.quality_score(decision)
      assert quality > 0.8
      assert quality <= 1.0
    end
  end

  describe "JSON encoding" do
    test "encodes to JSON" do
      {:ok, decision} = SchedulingDecision.new(module_id: "mod", node_id: "node")
      json = Jason.encode!(decision)
      assert is_binary(json)
      assert json =~ "\"module_id\":\"mod\""
    end
  end
end
