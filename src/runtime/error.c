#include "runtime.h"

#include <stdio.h>
#include <stdlib.h>

void cipr_runtime_oob(int64_t index, int64_t len) {
    fprintf(
        stderr,
        "Runtime Error: array index out of bounds (index=%lld, len=%lld)\n",
        (long long)index,
        (long long)len
    );
    exit(1);
}
