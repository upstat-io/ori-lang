# 100 Doors — idiomatic Python3 baseline for the interpreted perf gate.
#
# Same algorithm as ../ori/100_doors.ori simulate(): 100 doors, 100 passes,
# toggle every k-th door, one salt-driven extra toggle (salt % 100) so the
# result depends on the loop counter. main loops N times and prints the
# checksum (MUST equal the Ori microbench and the C++ baseline).


def simulate(salt: int) -> int:
    doors = [False] * 100
    for p in range(1, 101):
        for idx in range(p - 1, 100, p):
            doors[idx] = not doors[idx]
    s = salt % 100
    doors[s] = not doors[s]
    return sum(1 for d in doors if d)


def main() -> None:
    acc = 0
    for n in range(50000):
        acc += simulate(n)
    print(acc)


main()
