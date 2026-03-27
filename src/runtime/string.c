#include "runtime.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

int64_t cipr_str_len(cipr_str_t s) {
    return s.len;
}

cipr_str_t cipr_str_concat(cipr_str_t a, cipr_str_t b) {
    int64_t total = a.len + b.len;
    char *buf = malloc((size_t)total + 1);
    memcpy(buf, a.data, (size_t)a.len);
    memcpy(buf + a.len, b.data, (size_t)b.len);
    buf[total] = '\0';
    return (cipr_str_t){ .len = total, .data = buf };
}

int64_t cipr_str_eq(cipr_str_t a, cipr_str_t b) {
    return a.len == b.len && memcmp(a.data, b.data, (size_t)a.len) == 0;
}

cipr_str_t cipr_str_slice(cipr_str_t s, int64_t start, int64_t end) {
    if (start < 0) start = 0;
    if (end > s.len) end = s.len;
    if (start >= end) return (cipr_str_t){ .len = 0, .data = "" };
    int64_t len = end - start;
    char *buf = malloc((size_t)len + 1);
    memcpy(buf, s.data + start, (size_t)len);
    buf[len] = '\0';
    return (cipr_str_t){ .len = len, .data = buf };
}

int64_t cipr_str_to_int(cipr_str_t s) {
    return strtoll(s.data, NULL, 10);
}

double cipr_str_to_float(cipr_str_t s) {
    return strtod(s.data, NULL);
}

cipr_str_t cipr_int_to_str(int64_t n) {
    char buf[32];
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)n);
    char *data = malloc((size_t)len + 1);
    memcpy(data, buf, (size_t)len + 1);
    return (cipr_str_t){ .len = len, .data = data };
}

cipr_str_t cipr_float_to_str(double n) {
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%g", n);
    char *data = malloc((size_t)len + 1);
    memcpy(data, buf, (size_t)len + 1);
    return (cipr_str_t){ .len = len, .data = data };
}

int64_t cipr_str_contains(cipr_str_t s, cipr_str_t sub) {
    if (sub.len == 0) return 1;
    if (sub.len > s.len) return 0;
    for (int64_t i = 0; i <= s.len - sub.len; i++) {
        if (memcmp(s.data + i, sub.data, (size_t)sub.len) == 0) return 1;
    }
    return 0;
}

int64_t cipr_str_starts_with(cipr_str_t s, cipr_str_t prefix) {
    if (prefix.len > s.len) return 0;
    return memcmp(s.data, prefix.data, (size_t)prefix.len) == 0;
}

void cipr_str_free(cipr_str_t s) {
    free((void *)s.data);
}
