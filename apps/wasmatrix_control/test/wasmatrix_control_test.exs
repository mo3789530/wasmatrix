defmodule WasmatrixControlTest do
  use ExUnit.Case
  doctest WasmatrixControl

  test "control plane loads" do
    assert is_atom(WasmatrixControl)
  end
end
