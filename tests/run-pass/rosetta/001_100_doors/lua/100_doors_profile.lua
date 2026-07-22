-- 100 Doors -- characterization mirror of ./100_doors.lua.
--
-- Runs the SAME algorithm as simulate() over a reduced iteration count with a
-- counter incremented at every characterized operation site, and emits one
-- `work <category> <count>` line per counter. The harness runs this program in
-- its own untimed invocation, so the counters never enter a timed measurement.
--
-- The categories are the harness's FIXED language-independent vocabulary. Every
-- counter must equal its Ori and Python sibling exactly; the site list is
-- documented once in ../ori/100_doors_profile.ori.
--
-- Lua tables are 1-based, so Python's 0-based `range(p - 1, 100, p)` is written
-- here as `for idx = p, 100, p` and `doors[s]` as `doors[s + 1]`.

local ITERATIONS = 500

local function simulate_profiled(salt)
  local allocs = 0
  local ariths = 0
  local branches = 0
  local indexes = 0
  local iters = 0

  allocs = allocs + 1
  local doors = {}
  for i = 1, 100 do
    doors[i] = false
  end

  for p = 1, 100 do
    iters = iters + 1
    ariths = ariths + 1
    for idx = p, 100, p do
      iters = iters + 1
      indexes = indexes + 2
      doors[idx] = not doors[idx]
    end
  end

  ariths = ariths + 1
  local s = salt % 100
  indexes = indexes + 2
  doors[s + 1] = not doors[s + 1]

  local open = 0
  for i = 1, 100 do
    iters = iters + 1
    indexes = indexes + 1
    branches = branches + 1
    if doors[i] then
      ariths = ariths + 1
      open = open + 1
    end
  end

  return allocs, ariths, branches, indexes, iters, open
end

local function main()
  local calls = 0
  local allocs = 0
  local ariths = 0
  local branches = 0
  local indexes = 0
  local iters = 0
  local checksum = 0
  for n = 0, ITERATIONS - 1 do
    local a, ar, b, ix, it, c = simulate_profiled(n)
    calls = calls + 1
    allocs = allocs + a
    ariths = ariths + ar
    branches = branches + b
    indexes = indexes + ix
    iters = iters + it
    checksum = checksum + c
  end

  print(string.format("work alloc %d", allocs))
  print(string.format("work arith %d", ariths))
  print(string.format("work branch %d", branches))
  print(string.format("work call %d", calls))
  print("work field 0")
  print(string.format("work index %d", indexes))
  print(string.format("work loop_iter %d", iters))
  print("work string_op 0")
  print("work call_sites 1")
  print("work call_targets 1")
end

main()
