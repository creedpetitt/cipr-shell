#ifndef CIPR_RUNTIME_H
#define CIPR_RUNTIME_H

#include <stdint.h>

// Cipr fat-pointer string. The `data` pointer is always null-terminated.
typedef struct {
    int64_t len;
    const char *data;
} cipr_str_t;

typedef struct {
    void *fn_ptr;
    void *env_ptr;
} cipr_callable;

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

// --- net ---
int64_t    cipr_net_listen(int64_t port, int64_t nonblocking);
int64_t    cipr_net_accept(int64_t server_fd, int64_t nonblocking);
int64_t    cipr_net_connect(cipr_str_t host, int64_t port, int64_t nonblocking);
cipr_str_t cipr_net_read(int64_t fd, int64_t max_bytes);
int64_t    cipr_net_write(int64_t fd, cipr_str_t data);
void       cipr_net_close(int64_t fd);
cipr_str_t cipr_net_peer_ip(int64_t fd);

// --- http ---
void       cipr_http_start(int64_t port);
void       cipr_http_stop(void);
void       cipr_http_register(cipr_str_t method, cipr_str_t path, cipr_callable handler);
cipr_str_t cipr_http_method(void);
cipr_str_t cipr_http_path(void);
cipr_str_t cipr_http_body(void);
cipr_str_t cipr_http_query(cipr_str_t key, cipr_str_t def);
cipr_str_t cipr_http_param(cipr_str_t key, cipr_str_t def);
void       cipr_http_send(int64_t status, cipr_str_t content_type, cipr_str_t body);
void       cipr_http_json(int64_t status, cipr_str_t body);
void       cipr_http_file(cipr_str_t path);

#endif // CIPR_RUNTIME_H
