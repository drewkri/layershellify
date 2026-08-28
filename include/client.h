#pragma once
#include "protocols/wlr_layer_shell_unstable_v1_client.h"
#include <wayland-client-core.h>
#include <wayland-client-protocol.h>
#include <wayland-client.h>

struct ClientState {
  struct wl_display *display;
  struct wl_registry *reg;
  struct wl_compositor *compositor;
  struct zwlr_layer_shell_v1 *layer_shell;
  struct wl_shm *shm;

  // May in the future have multiple
  struct wl_surface *surface;
  struct zwlr_layer_surface_v1 *layer_surface;
};

struct ClientState init_client(void);
void disconnect_client(struct ClientState *);
