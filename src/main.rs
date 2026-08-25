use smithay::{
    backend::renderer::pixman::PixmanRenderer,
    reexports::{
        calloop::{self, EventLoop, Interest, generic::Generic},
        wayland_server::Display,
    },
};
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
    pub renderer: PixmanRenderer,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start client window
    let (client, mut event_queue) = open_window(Layer::Top, Anchor::LEFT);

    // Start compositor
    let display: Display<State> = Display::new().unwrap();
    let mut comp_event_loop: EventLoop<State> = EventLoop::try_new().unwrap();
    let server = ServerState::new(display, &mut comp_event_loop);

    comp_event_loop
        .handle()
        .insert_source(
            Generic::new(event_queue, Interest::BOTH, calloop::Mode::Level),
            move |_, queue, data| {
                // I think this *should* be outside of this function but there isn't an easy way to
                // make that work since the event loop now owns the queue
                let guard = queue.prepare_read().unwrap();
                guard.read().unwrap();

                // SAFETY: The event queue is alive throughout all of main.
                unsafe {
                    let queue_ref = queue.get_mut();
                    queue_ref.dispatch_pending(data).unwrap();
                    queue_ref.flush().unwrap();
                }
                Ok(calloop::PostAction::Continue)
            },
        )
        .unwrap();

    let mut state = State {
        server,
        client,
        should_close: false,
        renderer: PixmanRenderer::new().unwrap(),
    };

    comp_event_loop.run(None, &mut state, move |_| {
        // Called when an event occurs
        // Events are already passed to appropriate handlers,
        // so nothing needs to be here.
    })?;
    Ok(())
}
