use smithay::reexports::calloop::EventLoop;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, Layer};

use crate::{compositor::BoxComp, window::open_window};

mod compositor;
mod window;

fn main() {
    // Start client window
    let (mut state, mut event_queue) = open_window(Layer::Top, Anchor::LEFT);

    // Start compositor
    let mut comp_event_loop: EventLoop<BoxComp> = EventLoop::try_new().unwrap();
    let comp = BoxComp::new(&mut comp_event_loop);

    while !state.should_close() {
        event_queue.blocking_dispatch(&mut state).unwrap();
    }
}
