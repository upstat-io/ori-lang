/*
 * Semantic pin: deliberate heap-use-after-free.
 *
 * Compiled directly by scripts/sanitizer-smoke.sh with -fsanitize=address.
 * Without ASan: exits 0 (freed memory not yet reclaimed by OS).
 * With ASan: crashes with "heap-use-after-free" report.
 *
 * This is a C-only workaround because Ori's extern "c" from "lib" blocks
 * don't resolve in AOT mode (FFI not yet complete — see roadmap §11.12
 * for the upgrade to an Ori-native FFI semantic pin).
 */

#include <stdlib.h>

int main(void) {
    int *buf = (int *)malloc(16 * sizeof(int));
    if (!buf) return -1;

    /* Write a known pattern */
    for (int i = 0; i < 16; i++) {
        buf[i] = i * 42;
    }

    int saved = buf[7];  /* 7 * 42 = 294 */

    free(buf);

    /*
     * Use-after-free: read from the freed buffer.
     * Without ASan: typically returns 294 (memory not yet reclaimed).
     * With ASan: FATAL — heap-use-after-free detected.
     */
    volatile int result = buf[7];
    return (result == saved) ? 0 : 1;
}
