/*
 * Semantic pin helper: deliberate heap-use-after-free.
 *
 * This function allocates a buffer, writes to it, frees it, and then reads
 * from the freed buffer. Without sanitizers, the read usually succeeds
 * (freed memory not yet reclaimed by the OS). With ASan, the quarantined
 * memory is poisoned and the read triggers an immediate error.
 *
 * Compiled into libpin_helper.a by scripts/sanitizer-smoke.sh before
 * running the semantic pin test.
 */

#include <stdlib.h>
#include <string.h>

int trigger_use_after_free(void) {
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
