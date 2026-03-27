#include "runtime.h"
#include <time.h>

double cipr_time(void) {
#if defined(__linux__) || defined(__APPLE__)
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts) != 0) return 0.0;
    return (double)ts.tv_sec + ((double)ts.tv_nsec / 1e9);
#else
    return 0.0;
#endif
}
