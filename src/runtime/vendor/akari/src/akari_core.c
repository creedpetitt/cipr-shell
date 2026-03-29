#include "akari_core.h"
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>

static int set_nonblocking(int fd) {
    int flags = fcntl(fd, F_GETFL, 0);
    if (flags == -1) return -1;
    if (fcntl(fd, F_SETFL, flags | O_NONBLOCK) == -1) return -1;
    return 0;
}

int akari_tcp_init(int nonblocking) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd == -1) {
        AKARI_LOG("could not open socket");
        return -1;
    }
    
    if (nonblocking) {
        if (set_nonblocking(fd) == -1) {
            AKARI_LOG("could not set socket to non-blocking");
            close(fd);
            return -1;
        }
    }
    return fd;
}

int akari_tcp_bind(int fd, const struct sockaddr_in* addr) {
    int result = bind(fd, (struct sockaddr*)addr, sizeof *addr);
    if (result == -1) {
        AKARI_LOG("could not bind socket");
    }
    return result;
}

int akari_tcp_listen(int fd) {
    int result = listen(fd, SOMAXCONN);
    if (result == -1) {
        AKARI_LOG("could not listen to socket");
    }
    return result;
}

int akari_tcp_accept(int fd, struct sockaddr_in* addr, int nonblocking) {
    for (;;) {
        socklen_t addr_len = sizeof(struct sockaddr_in);
        int client_fd = accept(fd, (struct sockaddr*)addr, addr ? &addr_len : NULL);
        
        if (client_fd == -1 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            if (nonblocking) break;
            continue;
        }
        if (client_fd == -1 && (errno == EINTR || errno == ECONNABORTED)) {
            continue;
        }
        if (client_fd == -1) {
            AKARI_LOG("accept failed");
            return -1;
        }
        
        if (nonblocking) {
            if (set_nonblocking(client_fd) == -1) {
                AKARI_LOG("could not set client socket to non-blocking");
                close(client_fd);
                return -1;
            }
        }
        
        return client_fd;
    }
    return -1;
}

ssize_t akari_tcp_recv(int fd, void *buf, size_t size) {
    while (1) {
        ssize_t received = recv(fd, buf, size, 0);
        if (received == -1) {
            if (errno == EINTR) {
                continue;
            }
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                return 0;
            }
            AKARI_LOG("recv failed");
            return -1;
        }
        if (received == 0) {
            return -2;
        }
        return received;
    }
}

struct sockaddr_in akari_addr_init(const char* host, uint16_t port) {
    struct sockaddr_in addr = { 0 };
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    if (host == NULL) {
        addr.sin_addr.s_addr = htonl(INADDR_ANY);
    } else {
        if (inet_pton(AF_INET, host, &addr.sin_addr) <= 0) {
            AKARI_LOG("invalid address");
            addr.sin_family = 0;
        }
    }
    return addr;
}

int akari_tcp_start(uint16_t port) {
    int fd = akari_tcp_init(1); // Non-blocking for HTTP event loop
    if (fd == -1) {
        return -1;
    }
    int opt = 1;
    if (setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) == -1) {
        AKARI_LOG("setsockopt SO_REUSEADDR failed");
    }
    struct sockaddr_in addr = akari_addr_init(NULL, port);
    if (addr.sin_family == 0) {
        close(fd);
        return -1;
    }
    if (akari_tcp_bind(fd, &addr) == -1) {
        close(fd);
        return -1;
    }
    if (akari_tcp_listen(fd) == -1) {
        close(fd);
        return -1;
    }
    return fd;
}
