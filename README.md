# PLOX - Plugin Load Order eXpert

> Nexus link: <https://www.nexusmods.com/morrowind/mods/54262>

PLOX is a rust re-write of MLOX, a utility tool to sort a modlist topologically according to ordering rules. PLOX also outputs warnings and notes dependent on the mods in your load order. You can use it as a GUI app or as a commandline tool.

Rules are automatically downloaded from the respective Rules repository (see below for details).

Supported games:

- ✅TES3 - Morrowind (Windows, and Linux/macOS e.g. through Wine or Proton)
- ✅OpenMW (Windows, Linux, macOS)
- 🚧Cyberpunk 2077

Supported platforms: Windows, Linux (X11 and Wayland) and macOS.

## Configuration

The PLOX GUI reads its settings from `plox.toml`. A `plox.toml` in the current folder is used first; otherwise the file in the user config folder is used:

| Platform | User config folder |
| --- | --- |
| Windows | `%APPDATA%\plox\` |
| Linux | `~/.config/plox/` (`$XDG_CONFIG_HOME/plox/`) |
| macOS | `~/Library/Application Support/plox/` |

All fields are optional:

```toml
game = "Openmw"          # Morrowind, Openmw or Cyberpunk; detected if not set
no_rules_download = false
log_level = "info"
log_to_file = false      # writes plox.log next to plox.toml
ignore_warnings = false

[renderer]
backend = "auto"         # auto, glow or wgpu
ask_on_startup = true    # show the renderer dialog on every start
```

Rules are downloaded into an `mlox` (TES3/OpenMW) or `plox` (Cyberpunk) folder. If that folder exists in the current folder it is used; otherwise the rules are stored in the user data folder (`%APPDATA%\plox\`, `~/.local/share/plox/`, `~/Library/Application Support/plox/`).

### Game detection

If no game is set, PLOX checks in this order:

1. `Morrowind.exe` or `Morrowind.ini` in the current folder (any letter case) → Morrowind
2. `openmw.cfg` in the current folder → OpenMW
3. An `openmw.cfg` in the default OpenMW config folders → OpenMW
4. `bin/x64/Cyberpunk2077.exe` → Cyberpunk 2077

### OpenMW

PLOX resolves the full `openmw.cfg` chain like OpenMW does (`config=` entries, `?userconfig?` and similar tokens, `replace=`, quoted paths with `&&` escapes). An `openmw.cfg` in the current folder is used first, then `OPENMW_CONFIG` (path to a cfg file), `OPENMW_CONFIG_DIR`, the global config and finally the user config. A new load order is written only to the user `openmw.cfg`; other settings and parent configs are left untouched.

## Graphics requirements

The GUI has two renderers. At every start a dialog lets you pick one; tick "Don't ask again" to remember the choice (change it later with `--choose-renderer` or in `plox.toml`).

- **OpenGL (glow)**: needs OpenGL 2.1 or newer. Works on old GPUs and in VMs. If OpenGL fails, update the graphics driver, set `LIBGL_ALWAYS_SOFTWARE=1` on Linux, or use the command line tool.
- **wgpu**: Vulkan, DirectX 12 or Metal. Recommended for modern GPUs. If wgpu fails to start, PLOX restarts on OpenGL automatically, and offers OpenGL next time.

`auto` uses wgpu if a hardware Vulkan/DX12/Metal adapter is found, otherwise OpenGL.

GUI options:

```txt
Usage: plox_gui [OPTIONS]

Options:
      --renderer <RENDERER>  Renderer to use: auto, glow (OpenGL, legacy GPUs) or wgpu (Vulkan/DX12/Metal) [possible values: auto, glow, wgpu]
      --smoke-test           Render a few frames and exit, to check that the renderer works (no fallback)
      --choose-renderer      Show the renderer dialog even if "Don't ask again" was chosen
      --no-chooser           Never show the renderer dialog
  -h, --help                 Print help
  -V, --version              Print version
```

The renderer can also be set with the `PLOX_RENDERER` environment variable (`auto`, `glow`, `wgpu`).

Builds without wgpu (OpenGL only): `cargo build --release --no-default-features`.

### Linux

Runtime packages (Debian/Ubuntu names): `libxkbcommon-x11-0 libgl1 libegl1 libgl1-mesa-dri`, and for wgpu `libvulkan1 mesa-vulkan-drivers` (or your GPU vendor's Vulkan driver). Build packages: `libxkbcommon-dev libwayland-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libx11-dev`.

## Building

Requires Rust 1.95 or newer (the pinned toolchain is in `rust-toolchain.toml`).

```sh
cargo build --release --workspace
cargo test --workspace
```

## Rules

> Rules spec taken from [mlox - the elder scrolls Mod Load Order eXpert](https://github.com/mlox/mlox).

Plugins are sorted according to rules. For the rules spec see: [Rules Spec](./docs/Rules_spec.md)

The rules are hosted in their own repository:

- TES3 - Morrowind and OpenMW: <https://github.com/DanaePlays/mlox-rules>
- 🚧Cyberpunk 2077: <https://github.com/rfuzzo/cmop-rules>

**PRs are welcome!**

## Usage

### GUI

1. Download `plox_gui` (`plox_gui.exe` on Windows) from the latest release at <https://github.com/rfuzzo/plox/releases>
2. Morrowind: place it next to `Morrowind.exe`. OpenMW: place it anywhere; the OpenMW config is found automatically
3. Run `plox_gui`

### Commandline Interface

1. Download `plox` (`plox.exe` on Windows) from the latest release at <https://github.com/rfuzzo/plox/releases>
2. Morrowind: place it next to `Morrowind.exe`, or pass `--game-folder`. OpenMW: place it anywhere
3. Open a terminal window and run `plox` with a command

## Screenshots

![Screenshot](/assets/screenshot_gui1.png)
![Screenshot](/assets/screenshot_cli1.png)

## Credits

- [MLOX](https://github.com/mlox/mlox)
- [MLOX Rules](https://github.com/DanaePlays/mlox-rules)
- [openmw-config crate](https://github.com/DreamWeave-MP/Openmw_Config)
- [egui / eframe](https://github.com/emilk/egui)

## CLI Commands

```txt
Usage: plox [OPTIONS] <COMMAND>

Commands:
  sort    Sorts the current mod load order according to specified rules
  list    Lists the current mod load order
  verify  Verifies integrity of the specified rules
  graph   Outputs the rules as a graphviz dot file
  help    Print this message or the help of the given subcommand(s)

Options:
  -l, --log-level <LOG_LEVEL>  Set the log level, default is "info" [possible values: trace, debug, info, warn, error]
  -g, --game <GAME>            Set the game to evaluate, if no game is specified it will attempt to deduce the game from the current working directory [possible values: morrowind, openmw, cyberpunk]
  -n, --non-interactive        Disable user input
  -h, --help                   Print help
  -V, --version                Print version
```

### sort

Sorts the current mod load order according to specified rules

```txt
Usage: plox sort [OPTIONS]

Options:
  -g, --game-folder <GAME_FOLDER>  Root game folder (e.g. "Cyberpunk 2077" or "Morrowind"). Default is current working directory
  -r, --rules-dir <RULES_DIR>      Folder to read sorting rules from. Default is ./mlox (TES3) or ./plox if present, otherwise the plox data folder
  -d, --dry-run                    Just print the suggested load order without sorting
  -u, --unstable                   Use the potentially faster unstable sorter
  -n, --no-download                Disable automatic downloading of latest ruleset
  -m, --mod-list <MOD_LIST>        Read the input mods from a file instead of checking the root folder
  -h, --help                       Print help
```

### list

Lists the current mod load order

```txt
Usage: plox list [OPTIONS]

Options:
  -r, --root <ROOT>      Root game folder (e.g. "Cyberpunk 2077" or "Morrowind"). Default is current working directory
  -h, --help             Print help
```

### verify

Verifies integrity of the specified rules

```txt
Usage: plox verify [OPTIONS]

Options:
  -r, --rules-dir <RULES_DIR>  Folder to read sorting rules from. Default is ./mlox (TES3) or ./plox if present, otherwise the plox data folder
  -h, --help                   Print help
```

### graph

Outputs the rules as a graphviz dot file

```txt
Usage: plox graph [OPTIONS]

Options:
  -g, --game-folder <GAME_FOLDER>  Root game folder (e.g. "Cyberpunk 2077" or "Morrowind"). Default is current working directory
  -r, --rules-dir <RULES_DIR>      Folder to read sorting rules from. Default is ./mlox (TES3) or ./plox if present, otherwise the plox data folder
  -m, --mod-list <MOD_LIST>        Read the input mods from a file instead of checking the root folder
  -h, --help                       Print help
```
