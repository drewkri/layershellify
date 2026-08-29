#include "server.h"
#include "bridge.h"
#include <stdint.h>
#include <wayland-server-core.h>
#include <wayland-server-protocol.h>
#include <wayland-server.h>

static void compositor_create_region(struct wl_client *client,
                                     struct wl_resource *resource,
                                     uint32_t id) {}
static void compositor_create_surface(struct wl_client *client,
                                      struct wl_resource *resource,
                                      uint32_t id) {}
static void compositor_release(struct wl_client *client,
                               struct wl_resource *resource) {
  // Nothing needed here yet
}

static void compositor_handle_bind(struct wl_client *client, void *data,
                                   uint32_t version, uint32_t id) {
  struct State *state = data;
}

static const struct wl_compositor_interface comp_implementation = {
    .create_region = compositor_create_region,
    .create_surface = compositor_create_surface,
    .release = compositor_release,
};

/// Starts the server and creates a ServerState struct.
struct ServerState init_server(void) {
  struct ServerState state;

  state.display = wl_display_create();
  wl_display_init_shm(state.display);
  wl_display_add_shm_format(state.display, WL_SHM_FORMAT_ARGB8888);
  wl_display_add_shm_format(state.display, WL_SHM_FORMAT_XRGB8888);

  wl_display_add_socket_auto(state.display);

  return state;
}

/// Registers globals on the server and sets up callbacks that require
// access to *State
void server_start_handlers(struct State *state) {
  // Register globals
  wl_global_create(state->server_state.display, &wl_compositor_interface,
                   wl_compositor_interface.version, state,
                   compositor_handle_bind);
}
