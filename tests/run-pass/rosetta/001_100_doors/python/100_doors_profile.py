# 100 Doors -- characterization mirror of ./100_doors.py.
#
# Runs the SAME algorithm as simulate() over a reduced iteration count with a
# counter incremented at every characterized operation site, and emits one
# `work <category> <count>` line per counter. The harness runs this program in
# its own untimed invocation, so the counters never enter a timed measurement.
#
# The categories are the harness's FIXED language-independent vocabulary. Every
# counter must equal its Ori and LuaJIT sibling exactly; the site list is
# documented once in ../ori/100_doors_profile.ori.

ITERATIONS = 500


def simulate_profiled(salt: int) -> tuple[int, int, int, int, int, int]:
    allocs = 0
    ariths = 0
    branches = 0
    indexes = 0
    iters = 0

    allocs += 1
    doors = [False] * 100

    for p in range(1, 101):
        iters += 1
        ariths += 1
        for idx in range(p - 1, 100, p):
            iters += 1
            indexes += 2
            doors[idx] = not doors[idx]

    ariths += 1
    s = salt % 100
    indexes += 2
    doors[s] = not doors[s]

    open_count = 0
    for i in range(100):
        iters += 1
        indexes += 1
        branches += 1
        if doors[i]:
            ariths += 1
            open_count += 1

    return allocs, ariths, branches, indexes, iters, open_count


def main() -> None:
    calls = 0
    allocs = 0
    ariths = 0
    branches = 0
    indexes = 0
    iters = 0
    checksum = 0
    for n in range(ITERATIONS):
        a, ar, b, ix, it, c = simulate_profiled(n)
        calls += 1
        allocs += a
        ariths += ar
        branches += b
        indexes += ix
        iters += it
        checksum += c

    print(f"work alloc {allocs}")
    print(f"work arith {ariths}")
    print(f"work branch {branches}")
    print(f"work call {calls}")
    print("work field 0")
    print(f"work index {indexes}")
    print(f"work loop_iter {iters}")
    print("work string_op 0")
    print("work call_sites 1")
    print("work call_targets 1")


main()
