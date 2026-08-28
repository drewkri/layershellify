#include <stdlib.h>
#include <wayland-client-core.h>
#include <wayland-server-core.h>
#include <wayland-util.h>
#define PROJECT_NAME "layershellify"

#include "bridge.h"
#include "client.h"
#include "server.h"

int main(int argc, char **argv) {
  struct ClientState client_state = init_client();
  struct ServerState server_state = init_server();
  struct State *state = malloc(sizeof(struct State));
  state->client_state = client_state;
  state->server_state = server_state;

  while (!state->should_close) {
    // TODO: Something
    wl_display_dispatch(state->client_state.display);
  }

  // Cleanup (should be unnecessary since the program is going to stop anyway)
  // disconnect_client(&state->client_state);
  free(state);
  return 0;
}
