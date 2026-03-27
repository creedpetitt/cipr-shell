#include "runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

cipr_str_t cipr_readline(void) {
    char buf[4096];
    if (!fgets(buf, sizeof(buf), stdin)) return (cipr_str_t){ .len = 0, .data = "" };
    int64_t len = (int64_t)strlen(buf);
    if (len > 0 && buf[len - 1] == '\n') len--;
    char *data = malloc((size_t)len + 1);
    memcpy(data, buf, (size_t)len);
    data[len] = '\0';
    return (cipr_str_t){ .len = len, .data = data };
}
