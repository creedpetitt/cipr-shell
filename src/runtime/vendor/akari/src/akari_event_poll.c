#include "akari_internal.h"
#if defined(ESP_PLATFORM) || defined(AKARI_USE_LWIP)
#include "lwip/sockets.h"
#include <sys/poll.h>
#else
#include <poll.h>
#endif
#include <unistd.h>
#include <fcntl.h>

void akari_run_poll(int srv_fd, akari_callback on_data) {
    struct pollfd fds[AKARI_MAX_CONNECTIONS + 1];
    int nfds = 1;

    fds[0].fd = srv_fd;
    fds[0].events = POLLIN;

    for (int i = 1; i < AKARI_MAX_CONNECTIONS + 1; i++) {
        fds[i].fd = -1;
    }

    AKARI_LOG("poll engine started");

    while (akari_running) {
        // Clean up any FDs that might have been closed by sweep_timeouts or handle_write
        for (int i = 1; i < nfds; i++) {
            if (fds[i].fd != -1) {
                int flags = fcntl(fds[i].fd, F_GETFL, 0);
                if (flags == -1) {
                    fds[i].fd = -1;
                }
            }
        }

        int ready = poll(fds, nfds, 100);
        if (ready == -1) {
            if (akari_running) AKARI_LOG("poll failed");
            // If it still fails, sleep briefly to avoid spin lock and continue
            usleep(10000); 
            continue;
        }

        for (int i = 0; i < nfds; i++) {
            if (fds[i].revents == 0) continue;

            if (fds[i].fd == srv_fd) {
                struct sockaddr_in client_addr;
                int client_fd = akari_tcp_accept(srv_fd, &client_addr, 1);
                if (client_fd != -1) {
                    akari_connection* conn = akari_get_conn(client_fd);
                    if (conn) {
                        conn->client_ip = client_addr.sin_addr;
                    }
                    int added = 0;
                    for (int j = 1; j < AKARI_MAX_CONNECTIONS + 1; j++) {
                        if (fds[j].fd == -1) {
                            fds[j].fd = client_fd;
                            fds[j].events = POLLIN;
                            if (j >= nfds) nfds = j + 1;
                            added = 1;
                            break;
                        }
                    }
                    if (!added) {
                        AKARI_LOG("poll fds full");
                        akari_release_conn(client_fd);
                        close(client_fd);
                    }
                }
            } else {
                int client_fd = fds[i].fd;
                akari_connection* conn = akari_get_conn(client_fd);
                
                if (fds[i].revents & POLLIN) {
                    int status = akari_handle_client(client_fd, on_data);
                    if (status == -1 || (fds[i].revents & (POLLHUP | POLLERR))) {
                        close(client_fd);
                        fds[i].fd = -1;
                        continue;
                    }
                }
                
                if (conn && (fds[i].revents & POLLOUT)) {
                    akari_handle_write(conn);
                }
                
                if (conn && conn->fd != -1) {
                    fds[i].events = POLLIN;
                    if (conn->state == AKARI_CONN_SENDING) {
                        fds[i].events |= POLLOUT;
                    }
                } else if (conn && conn->fd == -1) {
                    fds[i].fd = -1;
                }
            }
        }
        akari_check_timers();
        akari_sweep_timeouts();
    }
}
