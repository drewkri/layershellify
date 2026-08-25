use smithay::{
    backend::renderer::damage::OutputDamageTracker,
    output::{self, PhysicalProperties},
    utils::Size,
};

use crate::State;

impl State {
    /// Sets up the compositor to begin displaying on the
    /// newly opened window.
    pub fn bridge_compositor(&mut self) {
        let refresh = self
            .client
            .output_state
            .outputs()
            .map_while(|output| self.client.output_state.info(&output))
            .map_while(|info| {
                info.modes
                    .iter()
                    .max_by(|a, b| a.refresh_rate.cmp(&b.refresh_rate))
                    .map(|mode| mode.refresh_rate)
            })
            .max()
            .unwrap_or(60);

        // Create a WlOutput and give it a Mode
        let size = Size::new(self.client.width as i32, self.client.height as i32);
        let mode = output::Mode { size, refresh };
        let output = output::Output::new(
            "LayerShell".to_owned(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: output::Subpixel::Unknown,
                make: "Compositor".to_owned(),
                model: "Window".to_owned(),
            },
        );

        let _global = output.create_global::<State>(self.server.display_handle());
        output.set_preferred(mode);
        // TODO: Set output scale
        output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
        self.server.space_mut().map_output(&output, (0, 0));

        // TODO: FIX!
        self.server.output_damage_tracker = Some(OutputDamageTracker::from_output(&output));
        self.server.output = Some(output);
    }
}
