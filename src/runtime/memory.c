#include "runtime.h"
#include <stdlib.h>
#include <stdint.h>

void* cipr_malloc(int64_t size) {
    return malloc((size_t)size);
}

void cipr_free(void* ptr) {
    free(ptr);
}
