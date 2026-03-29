#ifndef AKARI_INTERNAL_H
#define AKARI_INTERNAL_H

#include "akari_event.h"

void akari_run_epoll(int srv_fd, akari_callback on_data);
void akari_run_poll(int srv_fd, akari_callback on_data);

int akari_handle_client(int fd, akari_callback on_data);
void akari_release_conn(int fd);
void akari_check_timers(void);
void akari_sweep_timeouts(void);
void akari_send_error(int fd, int status, int keep_alive);

extern volatile int akari_running;

#endif
