#ifndef CIPR_RUNTIME_H
#define CIPR_RUNTIME_H

#include <stdint.h>

// Cipr fat-pointer string. The `data` pointer is always null-terminated.
typedef struct {
    int64_t len;
    const char *data;
} cipr_str_t;

// --- stdio ---
void cipr_print_str(cipr_str_t str);
void cipr_print_int(int64_t val);
void cipr_print_float(double val);
void cipr_print_bool(int64_t val);

// --- memory ---
void *cipr_malloc(int64_t size);
void  cipr_free(void *ptr);

// --- time ---
double cipr_time(void);

// --- string ---
int64_t    cipr_str_len(cipr_str_t s);
cipr_str_t cipr_str_concat(cipr_str_t a, cipr_str_t b);
int64_t    cipr_str_eq(cipr_str_t a, cipr_str_t b);
cipr_str_t cipr_str_slice(cipr_str_t s, int64_t start, int64_t end);
int64_t    cipr_str_to_int(cipr_str_t s);
double     cipr_str_to_float(cipr_str_t s);
cipr_str_t cipr_int_to_str(int64_t n);
cipr_str_t cipr_float_to_str(double n);
int64_t    cipr_str_contains(cipr_str_t s, cipr_str_t sub);
int64_t    cipr_str_starts_with(cipr_str_t s, cipr_str_t prefix);
void       cipr_str_free(cipr_str_t s);

// --- file ---
cipr_str_t cipr_fread_all(cipr_str_t path);
int64_t    cipr_fwrite(cipr_str_t path, cipr_str_t content);
int64_t    cipr_fappend(cipr_str_t path, cipr_str_t content);
int64_t    cipr_file_exists(cipr_str_t path);

// --- io ---
cipr_str_t cipr_readline(void);

#endif // CIPR_RUNTIME_H
