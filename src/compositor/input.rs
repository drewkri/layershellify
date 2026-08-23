use smithay::{
    input::SeatHandler, reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::seat::WaylandFocus,
};

use crate::State;

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;

    type PointerFocus = WlSurface;

    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {
        &mut self.server.seat_state
    }
}
