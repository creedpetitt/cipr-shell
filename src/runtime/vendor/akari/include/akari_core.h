#ifndef AKARI_CORE_H
#define AKARI_CORE_H

#include <stdint.h>
#include <stddef.h>

// PLATFORM DETECTION
#if defined(__linux__) || defined(__APPLE__)
    #include <sys/types.h>
    #include <netinet/in.h>
    #include <unistd.h>
    #include <arpa/inet.h>
    #include <sys/socket.h>
#elif defined(AKARI_USE_LWIP) || defined(ESP_PLATFORM)
    #include <sys/types.h>
    #include "lwip/sockets.h"
    #include "lwip/netdb.h"
#else
    typedef int32_t ssize_t;
    struct sockaddr_in {
        uint16_t sin_family;
        uint16_t sin_port;
        struct { uint32_t s_addr; } sin_addr;
    };
    #define AF_INET 2
    #define SOCK_STREAM 1
    #define SOMAXCONN 128
#endif

// --- PORTABILITY GUARDS ---
#ifndef SOMAXCONN
#define SOMAXCONN 128
#endif

#ifndef MSG_NOSIGNAL
#define MSG_NOSIGNAL 0
#endif

#ifndef AKARI_LOG
    #ifdef AKARI_DEBUG
        #include <stdio.h>
        #define AKARI_LOG(fmt, ...) printf("[AKARI] " fmt "\n", ##__VA_ARGS__)
    #else
        #define AKARI_LOG(fmt, ...) ((void)0)
    #endif
#endif

int akari_tcp_init(int nonblocking);
int akari_tcp_bind(int fd, const struct sockaddr_in* addr);
int akari_tcp_listen(int fd);
int akari_tcp_accept(int fd, struct sockaddr_in* addr, int nonblocking);
int akari_tcp_connect(const char* host, uint16_t port, int nonblocking);
int akari_tcp_start(uint16_t port);
struct sockaddr_in akari_addr_init(const char* host, uint16_t port);
ssize_t akari_tcp_recv(int fd, void *buf, size_t size);

#endif
