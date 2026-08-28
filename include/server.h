#pragma once
#include <wayland-client-core.h>
#include <wayland-server.h>
struct ServerState {
  struct wl_display *display;
};

struct ServerState init_server(void);
