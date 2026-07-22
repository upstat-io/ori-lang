-- 100 Doors -- pinned LuaJIT `-joff` baseline for the strict-interpreter gate.
--
-- Same algorithm as ../ori/100_doors.ori simulate() and ../python/100_doors.py:
-- 100 doors, 100 passes, toggle every k-th door, one salt-driven extra toggle
-- (salt % 100) so the result depends on the loop counter. main loops N times and
-- prints the checksum (MUST equal the Ori microbench, the Python baseline, and
-- the C++ baseline).
--
-- Lua tables are 1-based, so Python's 0-based `range(p - 1, 100, p)` is written
-- here as `for idx = p, 100, p` and `doors[s]` as `doors[s + 1]`.
-- Run with `-joff` to disable the JIT: this is the strict-interpreter comparator.

local function simulate(salt)
  local doors = {}
  for i = 1, 100 do
    doors[i] = false
  end
  for p = 1, 100 do
    for idx = p, 100, p do
      doors[idx] = not doors[idx]
    end
  end
  local s = salt % 100
  doors[s + 1] = not doors[s + 1]
  local open = 0
  for i = 1, 100 do
    if doors[i] then
      open = open + 1
    end
  end
  return open
end

local function main()
  local acc = 0
  for n = 0, 49999 do
    acc = acc + simulate(n)
  end
  print(acc)
end

main()
