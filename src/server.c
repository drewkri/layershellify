#include "server.h"
struct ServerState init_server(void) {
  struct ServerState state;

  state.display = wl_display_create();

  wl_display_add_socket_auto(state.display);

  return state;
}
