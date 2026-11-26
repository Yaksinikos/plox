# Changelog

All notable changes to this project will be documented in this file.

## v[0.6.0]

### Added

- Linux and macOS support for the GUI (X11 and Wayland) #36
- Selectable renderer: OpenGL (glow) for legacy GPUs, wgpu (Vulkan/DX12/Metal) for modern GPUs, with a startup dialog and "Don't ask again"
- `auto` renderer detection and automatic fallback to OpenGL when wgpu fails
- GUI options `--renderer`, `--choose-renderer`, `--no-chooser`, `--smoke-test` and the `PLOX_RENDERER` environment variable
- `plox.toml` and rules are read from the platform config/data folders if they are not in the current folder #33
- OpenMW config detection in the default OpenMW config folders
- CI for Windows, Linux and macOS with lint, MSRV and headless renderer checks
- Daily backup of `Morrowind.ini` / the user `openmw.cfg` before saving a new load order (`backup_load_order`, on by default; `plox sort --no-backup`)

### Changed

- Rust 2024 edition, minimum Rust version 1.95, strict lints
- egui/eframe 0.37 and updated dependencies
- OpenMW: the full `openmw.cfg` chain is resolved with `openmw-config`; the load order is saved only to the user `openmw.cfg`
- Order rules that contradict earlier rules are skipped with a warning instead of failing the whole sort
- Errors are reported instead of crashing

### Fixed

- OpenMW `data=` paths containing `&` (escaped as `&&`) #34
- Regex special characters such as `(`, `)` or `+` in plugin names of wildcard rules #37
- Morrowind: `Data Files`, `Morrowind.ini` and plugin names are matched case-insensitively (Linux)

## v[0.3.1]

### Fixed

- fixed a bug where desc would not match properly

## v[0.3.0]

### Added

- sort plugins also by masters (masters before dependent plugins)

### Changed

- Fixed OpenMW enum naming again

### Fixed

- fixed a bug where the VER predicate was not properly parsed

## v[0.2.2]

### Fixed

- Add omwgame support  #22
- Add omwscripts support  #24
- fixed plugin references log output as info

## v[0.2.1]

### Fixed

- fixed a bug where custom openmw cfg was not used

## v[0.2.0]

### Added

- added a settings file
- added openmw cfg overwrite
- added a log file

### Changed

- Fixed OpenMW naming

### Fixed

- fixed missing Predicates
- fixed a bug where openmw was not recognized properly

## v[0.1.0]

### Added

- Initial release
