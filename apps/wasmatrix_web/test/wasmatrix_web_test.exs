defmodule WasmatrixWebTest do
  use ExUnit.Case
  doctest WasmatrixWeb

  test "web interface loads" do
    assert is_atom(WasmatrixWeb)
  end
end
