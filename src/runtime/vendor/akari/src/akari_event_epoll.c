#include "akari_internal.h"
#include <sys/epoll.h>
#include <unistd.h>

void akari_run_epoll(int srv_fd, akari_callback on_data) {
    int epoll_fd = epoll_create1(0);
    if (epoll_fd == -1) {
        AKARI_LOG("epoll_create1 failed");
        return;
    }

    struct epoll_event ev, events[AKARI_MAX_CONNECTIONS];

    ev.events = EPOLLIN;
    ev.data.fd = srv_fd;
    if (epoll_ctl(epoll_fd, EPOLL_CTL_ADD, srv_fd, &ev) == -1) {
        AKARI_LOG("epoll_ctl server fd failed");
        close(epoll_fd);
        return;
    }

    AKARI_LOG("epoll engine started");

    while (akari_running) {
        int nfds = epoll_wait(epoll_fd, events, AKARI_MAX_CONNECTIONS, 100);
        if (nfds == -1) {
            if (akari_running) AKARI_LOG("epoll_wait failed");
            break;
        }
        for (int i = 0; i < nfds; i++) {
            if (events[i].data.fd == srv_fd) {
                struct sockaddr_in client_addr;
                int client_fd = akari_tcp_accept(srv_fd, &client_addr, 1);
                if (client_fd != -1) {
                    akari_connection* conn = akari_get_conn(client_fd);
                    if (conn) {
                        conn->client_ip = client_addr.sin_addr;
                        conn->epoll_flags = EPOLLIN;
                    }
                    ev.events = EPOLLIN;
                    ev.data.fd = client_fd;
                    if (epoll_ctl(epoll_fd, EPOLL_CTL_ADD, client_fd, &ev) == -1) {
                        AKARI_LOG("epoll_ctl client fd failed");
                        akari_release_conn(client_fd);
                        close(client_fd);
                    }
                }
            } else {
                int client_fd = events[i].data.fd;
                akari_connection* conn = akari_get_conn(client_fd);
                
                if (events[i].events & EPOLLIN) {
                    int status = akari_handle_client(client_fd, on_data);
                    if (status == -1) {
                        epoll_ctl(epoll_fd, EPOLL_CTL_DEL, client_fd, NULL);
                        close(client_fd);
                        continue;
                    }
                }
                
                if (conn && (events[i].events & EPOLLOUT)) {
                    akari_handle_write(conn);
                }
                
                // Update epoll mask
                if (conn && conn->fd != -1) {
                    uint8_t new_flags = EPOLLIN;
                    if (conn->state == AKARI_CONN_SENDING) {
                        new_flags |= EPOLLOUT;
                    }
                    if (conn->epoll_flags != new_flags) {
                        ev.events = new_flags;
                        ev.data.fd = client_fd;
                        epoll_ctl(epoll_fd, EPOLL_CTL_MOD, client_fd, &ev);
                        conn->epoll_flags = new_flags;
                    }
                }
            }
        }
        akari_check_timers();
        akari_sweep_timeouts();
    }

    close(epoll_fd);
}
