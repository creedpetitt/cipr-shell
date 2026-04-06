#include "runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

cipr_string_t *cipr_readline(void) {
    char buf[4096];
    if (!fgets(buf, sizeof(buf), stdin)) return cipr_string_new_empty();
    int64_t len = (int64_t)strlen(buf);
    if (len > 0 && buf[len - 1] == '\n') len--;
    return cipr_string_new_copy(buf, len);
}
