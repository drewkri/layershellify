#define _GNU_SOURCE
#include <asm-generic/errno-base.h>
#include <bits/sockaddr.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/errno.h>
#include <sys/poll.h>
#include <sys/socket.h>
#include <unistd.h>
// #include <wayland-client-core.h>
// #include <wayland-client-protocol.h>
// #include <wayland-server-core.h>
// #include <wayland-util.h>
#define PROJECT_NAME "layershellify"

// #include "bridge.h"
// #include "client.h"
// #include "server.h"

struct sockaddr_un {
  sa_family_t sun_family;
  // path should never be longer than 128 characters, surely
  char sun_data[128];
};

int main(int argc, char **argv) {
  // struct wl_display *display = wl_display_connect(NULL);
  // struct wl_registry *reg = wl_display_get_registry(display);
  // int client_fd = wl_display_get_fd(display);

  // struct wl_display *server_display = wl_display_create();
  // const char *socket_name = wl_display_add_socket_auto(server_display);
  // struct wl_event_loop *event_loop =
  // wl_display_get_event_loop(server_display); int server_fd =
  // wl_event_loop_get_fd(event_loop);

  const char *socket_name = "wayland-0";
  int server_fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
  struct sockaddr_un addr = {.sun_family = AF_UNIX}, client_addr;

  // // construct the address
  const char *separator = "/";
  const char *xdg_runtime = getenv("XDG_RUNTIME_DIR");
  if (!xdg_runtime) {
    return 1;
  }
  strcat(addr.sun_data, xdg_runtime);
  strcat(addr.sun_data, separator);
  strcat(addr.sun_data, socket_name);

  int opt = 1;
  if (setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
    perror("setsockopt SO_REUSEADDR failed");
    exit(EXIT_FAILURE);
  }

  if (bind(server_fd, (struct sockaddr *)&addr, strlen(addr.sun_data)) < 0) {
    printf("bind err, %d\n", errno);
    close(server_fd);
    return 1;
  }

  // We only want 1 client and will not accept any later connection attempts.
  // (For now, anyway. I'd assume it's only one client needed.)
  if (listen(server_fd, 1) < 0) {
    printf("listen err\n");
    close(server_fd);
    return 1;
  };

  // Blocks until a client connects.
  printf("Waiting for client\n");
  int server_client_fd = accept4(server_fd, NULL, NULL, SOCK_CLOEXEC);
  if (server_client_fd < 0) {
    printf("accept err: %d\n", errno);
    close(server_fd);
    return 1;
  }
  printf("accepted client\n");

  struct pollfd fds[2];
  fds[0].fd = server_client_fd;
  fds[0].events = POLLIN;
  fds[1].fd = server_fd;
  fds[1].events = POLLIN;

  printf("Client: %d, Server: %d\n", fds[0].fd, fds[1].fd);

  printf("succeeded\n");
  // while (true) {
  //   int res = poll(fds, 2, 500);
  //   if (res > 0) {
  //     printf("got an event\n");
  //     // Read from one fd, write to the other
  //     for (int i = 0; i <= 1; i++) {
  //       if (fds[i].revents & POLLIN) {
  //         printf("checking: %d\n", i);
  //         printf("doing the thing\n");
  //         char buffer[256];
  //         ssize_t bytes_read = read(fds[i].fd, buffer, sizeof(buffer));
  //         printf("read bytes: %d, if err see: %d\n", bytes_read, errno);
  //         if (bytes_read > 0) {
  //           printf("Read event triggered! Received: \"%s\"\n", buffer);
  //         }
  //         write(fds[(i + 1) % 2].fd, buffer, bytes_read);
  //       }
  //     }
  //   }
  // }
  close(server_client_fd);
  close(server_fd);
}
