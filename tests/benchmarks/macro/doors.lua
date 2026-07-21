-- 100 doors simulation. Equal-work port of doors.ori / doors.py.
-- Run under LuaJIT with -joff (strict-interpreter north-star comparator).
local function simulate()
  local doors = {}
  for i = 1, 100 do doors[i] = false end
  for p = 1, 100 do
    for idx = p, 100, p do
      doors[idx] = not doors[idx]
    end
  end
  local open = 0
  for i = 1, 100 do
    if doors[i] then open = open + 1 end
  end
  return open
end

local total = 0
for _ = 1, 200 do
  total = total + simulate()
end
print(total)
