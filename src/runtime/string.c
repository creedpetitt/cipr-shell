#include "runtime.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

cipr_str_t cipr_empty_str(void) {
    return (cipr_str_t){ .len = 0, .data = "" };
}

cipr_string_t *cipr_string_new_empty(void) {
    cipr_string_t *s = malloc(sizeof(cipr_string_t));
    if (!s) return NULL;
    s->view = cipr_empty_str();
    return s;
}

cipr_string_t *cipr_string_new_copy(const char *data, int64_t len) {
    if (!data || len <= 0) {
        return cipr_string_new_empty();
    }

    char *copy = malloc((size_t)len + 1);
    if (!copy) {
        return cipr_string_new_empty();
    }

    memcpy(copy, data, (size_t)len);
    copy[len] = '\0';
    return cipr_string_new_owned(copy, len);
}

cipr_string_t *cipr_string_new_owned(char *owned_data, int64_t len) {
    cipr_string_t *s = malloc(sizeof(cipr_string_t));
    if (!s) {
        free(owned_data);
        return NULL;
    }

    if (!owned_data || len <= 0) {
        free(owned_data);
        s->view = cipr_empty_str();
        return s;
    }

    s->view = (cipr_str_t){ .len = len, .data = owned_data };
    return s;
}

int64_t cipr_str_len(cipr_str_t s) {
    return s.len;
}

cipr_string_t *cipr_str_concat(cipr_str_t a, cipr_str_t b) {
    int64_t total = a.len + b.len;
    char *buf = malloc((size_t)total + 1);
    if (!buf) {
        return cipr_string_new_empty();
    }
    memcpy(buf, a.data, (size_t)a.len);
    memcpy(buf + a.len, b.data, (size_t)b.len);
    buf[total] = '\0';
    return cipr_string_new_owned(buf, total);
}

int64_t cipr_str_eq(cipr_str_t a, cipr_str_t b) {
    return a.len == b.len && memcmp(a.data, b.data, (size_t)a.len) == 0;
}

cipr_string_t *cipr_str_slice(cipr_str_t s, int64_t start, int64_t end) {
    if (start < 0) start = 0;
    if (end > s.len) end = s.len;
    if (start >= end) return cipr_string_new_empty();
    int64_t len = end - start;
    char *buf = malloc((size_t)len + 1);
    if (!buf) {
        return cipr_string_new_empty();
    }
    memcpy(buf, s.data + start, (size_t)len);
    buf[len] = '\0';
    return cipr_string_new_owned(buf, len);
}

int64_t cipr_str_to_int(cipr_str_t s) {
    return strtoll(s.data, NULL, 10);
}

double cipr_str_to_float(cipr_str_t s) {
    return strtod(s.data, NULL);
}

cipr_string_t *cipr_int_to_str(int64_t n) {
    char buf[32];
    int len = snprintf(buf, sizeof(buf), "%lld", (long long)n);
    return cipr_string_new_copy(buf, (int64_t)len);
}

cipr_string_t *cipr_float_to_str(double n) {
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%g", n);
    return cipr_string_new_copy(buf, (int64_t)len);
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

void cipr_string_free(cipr_string_t *s) {
    if (!s) return;
    if (s->view.data && s->view.len > 0) {
        free((void *)s->view.data);
    }
    free(s);
}
