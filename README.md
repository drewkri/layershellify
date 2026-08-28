# Layershellify

A tool to put Wayland windows onto a layer shell surface.

## Building

Run `meson setup build` and then `meson compile` in the new `build` directory.
Requires libwayland to be installed (including wayland-scanner which is needed for the build process).
Wlr protocols are expected to be in /usr/share/wlr-protocols, which is where the arch package wlr-protocols installs them to.
They are requried to build.
