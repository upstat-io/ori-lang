# 100 Doors -- work-count mirror of ./100_doors.py for the comparator pin.
#
# Runs the SAME algorithm as simulate() with a counter incremented at every
# counted operation site, over a reduced iteration count, and emits one
# `work <key> <count>` line per counter. The harness runs this program in its
# own untimed invocation, so the counters never enter a timed measurement.
#
# Every counter must equal its Ori and LuaJIT sibling exactly.

ITERATIONS = 500


def simulate_counted(salt: int) -> tuple[int, int, int, int]:
    doors = [False] * 100
    passes = 0
    toggles = 0
    reads = 0
    for p in range(1, 101):
        passes += 1
        for idx in range(p - 1, 100, p):
            toggles += 1
            doors[idx] = not doors[idx]
    s = salt % 100
    toggles += 1
    doors[s] = not doors[s]

    open_count = 0
    for i in range(100):
        reads += 1
        open_count += 1 if doors[i] else 0

    return passes, toggles, reads, open_count


def main() -> None:
    calls = 0
    passes = 0
    toggles = 0
    reads = 0
    checksum = 0
    for n in range(ITERATIONS):
        p, t, r, c = simulate_counted(n)
        calls += 1
        passes += p
        toggles += t
        reads += r
        checksum += c

    print(f"work calls {calls}")
    print(f"work passes {passes}")
    print(f"work toggles {toggles}")
    print(f"work reads {reads}")
    print(f"work checksum {checksum}")


main()
