// 100 Doors — C++ baseline for the Ori microbenchmark (PLAY/EXPERIMENT).
//
// Same algorithm as ../ori/100_doors.ori simulate():
//   - 100 doors, 100 passes, toggle every k-th door
//   - one extra salt-driven toggle (salt % 100) so the result depends on
//     the loop counter, defeating -O3 hoisting of the invariant call
//   - main loops N times and prints the checksum (MUST equal Ori's)
//
// "Efficient C++": stack array, zero heap allocation. This is the point
// of the comparison — Ori's idiomatic [bool] list allocates per call;
// the gap (if any) is exactly what AIMS must prove it elides.

#include <cstdio>
#include <array>

static int simulate(int salt) {
    std::array<bool, 100> doors{};  // all false
    for (int pass = 1; pass <= 100; ++pass) {
        for (int idx = pass - 1; idx < 100; idx += pass) {
            doors[idx] = !doors[idx];
        }
    }
    int s = salt % 100;
    doors[s] = !doors[s];

    int count = 0;
    for (int i = 0; i < 100; ++i) {
        if (doors[i]) ++count;
    }
    return count;
}

int main() {
    long acc = 0;
    for (int n = 0; n < 50000; ++n) {
        acc += simulate(n);
    }
    std::printf("%ld\n", acc);
    return 0;
}
