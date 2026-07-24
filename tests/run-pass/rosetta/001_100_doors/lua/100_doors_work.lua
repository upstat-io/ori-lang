-- 100 Doors -- work-count mirror of ./100_doors.lua for the comparator pin.
--
-- Runs the SAME algorithm as simulate() with a counter incremented at every
-- counted operation site, over a reduced iteration count, and emits one
-- `work <key> <count>` line per counter. The harness runs this program in its
-- own untimed invocation, so the counters never enter a timed measurement.
--
-- Every counter must equal its Ori and Python sibling exactly.
--
-- Lua tables are 1-based, so Python's 0-based `range(p - 1, 100, p)` is written
-- here as `for idx = p, 100, p` and `doors[s]` as `doors[s + 1]`.

local ITERATIONS = 500

local function simulate_counted(salt)
  local doors = {}
  for i = 1, 100 do
    doors[i] = false
  end
  local passes = 0
  local toggles = 0
  local reads = 0
  for p = 1, 100 do
    passes = passes + 1
    for idx = p, 100, p do
      toggles = toggles + 1
      doors[idx] = not doors[idx]
    end
  end
  local s = salt % 100
  toggles = toggles + 1
  doors[s + 1] = not doors[s + 1]

  local open = 0
  for i = 1, 100 do
    reads = reads + 1
    if doors[i] then
      open = open + 1
    end
  end

  return passes, toggles, reads, open
end

local function main()
  local calls = 0
  local passes = 0
  local toggles = 0
  local reads = 0
  local checksum = 0
  for n = 0, ITERATIONS - 1 do
    local p, t, r, c = simulate_counted(n)
    calls = calls + 1
    passes = passes + p
    toggles = toggles + t
    reads = reads + r
    checksum = checksum + c
  end

  print(string.format("work calls %d", calls))
  print(string.format("work passes %d", passes))
  print(string.format("work toggles %d", toggles))
  print(string.format("work reads %d", reads))
  print(string.format("work checksum %d", checksum))
end

main()
