# grazz

grazz is a desktop toy and idle game written in Rust that renders procedurally animated grass at the bottom of your screen using Wayland's layer-shell protocol. It features a GPU-accelerated grass simulation, a persistent progression system, and an IPC interface for external control.

## Features

* **Layer Shell Integration**: Renders as a bottom-layer overlay across all connected monitors, sitting behind your windows but above your wallpaper.
* **WGPU Graphics**: Uses WGSL shaders to handle thousands of grass instances with realistic wind sway and cutting animations.
* **Persistent Progression**: Tracks your total grass cut, money earned, and upgrade levels in a local `save.json` file.
* **IPC Control**: Includes a Unix socket listener (`/tmp/grazz_ipc.sock`) to allow external scripts or keybinds to trigger the mower or buy upgrades.

## Requirements

* **Display Server**: Wayland (specifically compositors supporting the Layer Shell protocol, such as Sway or Hyprland).
* **Graphics**: A GPU supporting Vulkan, Metal, or DX12 (via `wgpu`).

## Getting Started

### Installation

Clone, build the project using cargo and run it.

```bash
cargo build --release

./target/release/grazz

```

The grass will begin growing at the bottom of your display. Initially, it grows slowly; you can upgrade your fertilizer via the IPC to speed this up.

## IPC Commands

You can interact with **grazz** by sending text commands to the Unix socket located at `/tmp/grazz_ipc.sock`.

| Command | Description |
| --- | --- |
| `MOW` | Dispatches the mower to sweep across the screen and collect money. |
| `BALANCE` | Returns your current money balance. |
| `UP_FERT` | Purchases a fertilizer upgrade (increases growth speed). |
| `UP_MOWER` | Purchases a mower upgrade (increases mower movement speed). |
| `UP_MONEY` | Purchases a money upgrade (increases yield per blade of grass). |
| `STATE` | Returns the full current game state in JSON format. |

**Cli example**

```bash
echo "MOW" | nc -U /tmp/grazz_ipc.sock

```

## Example ironbar integration

```json
{
  "type": "label",
  "label": "Grass {{1000:echo 'BALANCE' | nc -q 0 -U /tmp/grazz_ipc.sock}}"
}

```

## Technical Overview

* **Rendering**: The project uses an instanced rendering approach, drawing thousands of blades of grass from a single vertex buffer.
* **Shaders**: The `grass.wgsl` vertex shader handles individual blade height, wind phase, and the "cut" state based on the mower's position.
* **State Management**: Game data is stored in the user's config directory (e.g., `~/.config/grazz/save.json` on Linux) using `serde`.
