#include "runtime.h"

#include "vendor/akari/include/akari_core.h"

#include <arpa/inet.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int64_t cipr_net_listen(int64_t port, int64_t nonblocking) {
    if (port <= 0 || port > 65535) {
        return -1;
    }
    
    int fd = akari_tcp_init((int)nonblocking);
    if (fd == -1) return -1;

    int opt = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    struct sockaddr_in addr = akari_addr_init(NULL, (uint16_t)port);
    if (akari_tcp_bind(fd, &addr) == -1 || akari_tcp_listen(fd) == -1) {
        close(fd);
        return -1;
    }
    return (int64_t)fd;
}

int64_t cipr_net_accept(int64_t server_fd, int64_t nonblocking) {
    if (server_fd < 0) {
        return -1;
    }
    struct sockaddr_in addr;
    int client_fd = akari_tcp_accept((int)server_fd, &addr, (int)nonblocking);
    return (int64_t)client_fd;
}

int64_t cipr_net_connect(cipr_str_t host, int64_t port, int64_t nonblocking) {
    if (port <= 0 || port > 65535) return -1;
    return (int64_t)akari_tcp_connect(host.data, (uint16_t)port, (int)nonblocking);
}

cipr_string_t *cipr_net_read(int64_t fd, int64_t max_bytes) {
    if (fd < 0 || max_bytes <= 0) {
        return cipr_string_new_empty();
    }

    size_t cap = (size_t)max_bytes;
    char *buf = malloc(cap + 1);
    if (!buf) {
        return cipr_string_new_empty();
    }

    ssize_t n = recv((int)fd, buf, cap, 0);
    if (n <= 0) {
        free(buf);
        if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR)) {
            return cipr_string_new_empty();
        }
        return cipr_string_new_empty();
    }

    buf[n] = '\0';
    return cipr_string_new_owned(buf, (int64_t)n);
}

int64_t cipr_net_write(int64_t fd, cipr_str_t data) {
    if (fd < 0 || data.len < 0) {
        return -1;
    }

    size_t total = (size_t)data.len;
    size_t sent_total = 0;
    while (sent_total < total) {
        ssize_t sent = send((int)fd, data.data + sent_total, total - sent_total, MSG_NOSIGNAL);
        if (sent > 0) {
            sent_total += (size_t)sent;
            continue;
        }
        if (sent == -1 && (errno == EINTR)) {
            continue;
        }
        if (sent == -1 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            break;
        }
        return -1;
    }

    return (int64_t)sent_total;
}

void cipr_net_close(int64_t fd) {
    if (fd >= 0) {
        close((int)fd);
    }
}

cipr_string_t *cipr_net_peer_ip(int64_t fd) {
    if (fd < 0) {
        return cipr_string_new_empty();
    }

    struct sockaddr_in addr;
    socklen_t addr_len = sizeof(addr);
    if (getpeername((int)fd, (struct sockaddr *)&addr, &addr_len) != 0) {
        return cipr_string_new_empty();
    }

    char ip[INET_ADDRSTRLEN];
    if (!inet_ntop(AF_INET, &addr.sin_addr, ip, sizeof(ip))) {
        return cipr_string_new_empty();
    }

    size_t len = strlen(ip);
    char *copy = malloc(len + 1);
    if (!copy) {
        return cipr_string_new_empty();
    }
    memcpy(copy, ip, len + 1);
    return cipr_string_new_owned(copy, (int64_t)len);
}
