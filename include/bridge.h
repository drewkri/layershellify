#pragma once
#include "client.h"
#include "server.h"

struct State {
  struct ClientState client_state;
  struct ServerState server_state;
  bool should_close;
};
