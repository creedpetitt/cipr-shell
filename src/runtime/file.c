#include "runtime.h"
#include <stdio.h>
#include <stdlib.h>

// All cipr_str_t.data pointers are guaranteed null-terminated, so they can be
// passed directly to fopen/fseek etc. without an extra copy.

cipr_string_t *cipr_fread_all(cipr_str_t path) {
    FILE *f = fopen(path.data, "r");
    if (!f) return cipr_string_new_empty();

    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);

    char *buf = malloc((size_t)size + 1);
    if (!buf) {
        fclose(f);
        return cipr_string_new_empty();
    }
    int64_t read = (int64_t)fread(buf, 1, (size_t)size, f);
    buf[read] = '\0';
    fclose(f);
    return cipr_string_new_owned(buf, read);
}

int64_t cipr_fwrite(cipr_str_t path, cipr_str_t content) {
    FILE *f = fopen(path.data, "w");
    if (!f) return -1;
    size_t written = fwrite(content.data, 1, (size_t)content.len, f);
    fclose(f);
    return (written == (size_t)content.len) ? 0 : -1;
}

int64_t cipr_fappend(cipr_str_t path, cipr_str_t content) {
    FILE *f = fopen(path.data, "a");
    if (!f) return -1;
    size_t written = fwrite(content.data, 1, (size_t)content.len, f);
    fclose(f);
    return (written == (size_t)content.len) ? 0 : -1;
}

int64_t cipr_file_exists(cipr_str_t path) {
    FILE *f = fopen(path.data, "r");
    if (!f) return 0;
    fclose(f);
    return 1;
}
