use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_output,
    desktop::PopupKind,
    reexports::wayland_server::{Client, protocol::wl_surface::WlSurface},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent,
            is_sync_subsurface, with_states,
        },
        output::OutputHandler,
        shell::xdg::{XdgShellHandler, XdgToplevelSurfaceData},
        shm::ShmHandler,
    },
};
use smithay_client_toolkit::delegate_dispatch2;

use crate::compositor::{BoxComp, ClientState};

impl CompositorHandler for BoxComp {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        // Updates buffer
        on_commit_buffer_handler::<WlSurface>(surface);
        // Calls on_commit on the appropriate window, copied from Smallvil
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == &root)
            {
                window.on_commit();
            }
        };

        // Handle initial XdgShell configure. Copied from Smallvil.
        if let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().is_some_and(|tl| tl.wl_surface() == surface))
        {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });

            if !initial_configure_sent {
                window.toplevel().unwrap().send_configure();
            }
        }

        // Handle popup commits. Also copied from Smallvil.
        self.popups.commit(surface);
        if let Some(popup) = self.popups.find_popup(surface) {
            match popup {
                PopupKind::Xdg(ref xdg) => {
                    if !xdg.is_initial_configure_sent() {
                        // NOTE: This should never fail as the initial configure is always
                        // allowed.
                        xdg.send_configure().expect("initial configure failed");
                    }
                }
                PopupKind::InputMethod(ref _input_method) => {}
            }
        }
    }
}

impl ShmHandler for BoxComp {
    fn shm_state(&self) -> &smithay::wayland::shm::ShmState {
        &self.shm_state
    }
}

impl BufferHandler for BoxComp {
    fn buffer_destroyed(
        &mut self,
        buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer,
    ) {
        // nothing necessary here
    }
}

impl XdgShellHandler for BoxComp {
    fn xdg_shell_state(&mut self) -> &mut smithay::wayland::shell::xdg::XdgShellState {
        &mut self.shell_state
    }

    fn new_toplevel(&mut self, surface: smithay::wayland::shell::xdg::ToplevelSurface) {
        todo!()
    }

    fn new_popup(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: smithay::wayland::shell::xdg::PositionerState,
    ) {
        todo!()
    }

    fn grab(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        serial: smithay::utils::Serial,
    ) {
        todo!()
    }

    fn reposition_request(
        &mut self,
        surface: smithay::wayland::shell::xdg::PopupSurface,
        positioner: smithay::wayland::shell::xdg::PositionerState,
        token: u32,
    ) {
        todo!()
    }
}

impl OutputHandler for BoxComp {}

delegate_dispatch2!(BoxComp);
