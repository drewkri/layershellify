project(
  'layershellify',
  'c',
  meson_version : '>= 1.8.0',
  version : '0.1',
  default_options : ['warning_level=3'],
)

dependencies = [
    dependency('wayland-client'),
    dependency('wayland-server')
]


sources = [
  'src/main.c',
  'src/client.c',
  'src/server.c',
  'src/protocols/xdg_shell.c',
  'src/protocols/wlr_layer_shell_unstable_v1.c',
]

exe = executable(
  'layershellify',
  sources,
  dependencies : dependencies,
  install : true,
  include_directories: include_directories('include')
)

test('basic', exe)
