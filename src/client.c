#include "client.h"
#include "protocols/wlr_layer_shell_unstable_v1_client.h"
#include "shm_pool_alloc.c"
#include <stdint.h>
#include <string.h>
#include <wayland-client-core.h>
#include <wayland-client-protocol.h>
#include <wayland-server-protocol.h>

static void reg_global(void *data, struct wl_registry *reg, uint32_t name,
                       const char *interface, uint32_t version) {
  struct ClientState *state = data;
  if (strcmp(interface, wl_compositor_interface.name) == 0) {
    state->compositor =
        wl_registry_bind(reg, name, &wl_compositor_interface, version);
  } else if (strcmp(interface, zwlr_layer_shell_v1_interface.name) == 0) {
    state->layer_shell =
        wl_registry_bind(reg, name, &zwlr_layer_shell_v1_interface, version);
  } else if (strcmp(interface, wl_shm_interface.name) == 0) {
    state->shm = wl_registry_bind(reg, name, &wl_shm_interface, version);
  }
  // } else if (strcmp(interface, wl_compositor_interface.name) == 0) {
  // } else if (strcmp(interface, wl_compositor_interface.name) == 0) {
  // } else if (strcmp(interface, wl_compositor_interface.name) == 0) {
  // }
}
static void reg_global_remove(void *data, struct wl_registry *reg,
                              uint32_t name) {
  // no-op
}

static const struct wl_registry_listener registry_listener = {
    .global = &reg_global,
    .global_remove = &reg_global_remove,
};

/// Starts the client and creates a ClientState struct.
struct ClientState init_client(void) {
  struct ClientState state;

  state.display = wl_display_connect(NULL);
  state.reg = wl_display_get_registry(state.display);

  wl_registry_add_listener(state.reg, &registry_listener, &state);
  wl_display_roundtrip(state.display);

  state.surface = wl_compositor_create_surface(state.compositor);

  // TODO: Let's try to avoid allocating any memory here at all - I just want to
  // use the client's memory.
  // If necessary I will copy to a second buffer - may cause bad redraw timing
  // but I think it won't
  // const int width = 128, height = 128; const int stride
  // = width * 4, shm_pool_size = stride * height * 2; int fd =
  // allocate_shm_file(shm_pool_size); uint8_t *pool_data =
  //     mmap(NULL, shm_pool_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);

  // TODO: Choose output
  // Init layer surface
  state.layer_surface = zwlr_layer_shell_v1_get_layer_surface(
      state.layer_shell, state.surface, NULL, ZWLR_LAYER_SHELL_V1_LAYER_TOP,
      "layershellify");
  zwlr_layer_surface_v1_set_anchor(state.layer_surface,
                                   ZWLR_LAYER_SURFACE_V1_ANCHOR_LEFT);
  zwlr_layer_surface_v1_set_keyboard_interactivity(
      state.layer_surface,
      ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_ON_DEMAND);

  return state;
}

/// Cleanup resources. To be honest, the program is going to end, so I don't
/// know why we'd even bother calling this.
void disconnect_client(struct ClientState *state) {
  wl_display_disconnect(state->display);
  wl_registry_destroy(state->reg);
}
