use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, Layer};

use crate::{
    compositor::ServerState,
    window::{ClientState, open_window},
};

mod compositor;
mod window;

pub struct State {
    pub server: ServerState,
    pub client: ClientState,
    pub should_close: bool,
}

fn main() {
    // Start client window
    let (client, mut event_queue) = open_window(Layer::Top, Anchor::LEFT);

    // Start compositor
    let display: Display<State> = Display::new().unwrap();
    let mut comp_event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
    let server = ServerState::new(display, &mut comp_event_loop);

    let mut state = State {
        server,
        client,
        should_close: false,
    };

    while !state.should_close {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }
}
