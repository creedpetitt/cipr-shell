#ifndef AKARI_EVENT_H
#define AKARI_EVENT_H

#include "akari_core.h"

#ifndef AKARI_MAX_CONNECTIONS
#define AKARI_MAX_CONNECTIONS 8
#endif

#ifndef AKARI_REQ_BUF_SIZE
#define AKARI_REQ_BUF_SIZE 4096
#endif

#ifndef AKARI_RES_BUF_SIZE
#define AKARI_RES_BUF_SIZE 512
#endif

#ifndef AKARI_HEADER_TIMEOUT_MS
#define AKARI_HEADER_TIMEOUT_MS 5000
#endif

#ifndef AKARI_BODY_TIMEOUT_MS
#define AKARI_BODY_TIMEOUT_MS 10000
#endif

#ifndef AKARI_KEEPALIVE_TIMEOUT_MS
#define AKARI_KEEPALIVE_TIMEOUT_MS 10000
#endif

typedef enum {
    AKARI_CONN_IDLE,
    AKARI_CONN_READING_HEADERS,
    AKARI_CONN_READING_BODY,
    AKARI_CONN_DISPATCH,
    AKARI_CONN_SENDING
} akari_parse_state;

typedef struct {
    int fd;
    char buf[AKARI_REQ_BUF_SIZE];
    size_t buf_len;
    akari_parse_state state;
    uint64_t last_activity_ms;
    size_t parsed_header_len;
    size_t expected_body_len;
    struct in_addr client_ip;
    
    char res_buf[AKARI_RES_BUF_SIZE];
    size_t tx_len;
    size_t tx_sent;
    int tx_file_fd;
    size_t tx_file_len;
    size_t tx_file_sent;
    const uint8_t* tx_flash_buf;
    size_t tx_flash_len;
    size_t tx_flash_sent;
    int tx_keep_alive;
    uint8_t epoll_flags;
} akari_connection;

typedef void (*akari_callback)(akari_connection* conn);
typedef void (*akari_timer_callback)(void);

akari_connection* akari_get_conn(int fd);
void akari_handle_write(akari_connection* conn);
void akari_run_server(uint16_t port, akari_callback on_data);
void akari_stop(void);
void akari_add_timer(akari_timer_callback cb, int interval_ms);

#endif
