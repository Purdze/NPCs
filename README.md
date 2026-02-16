# NPCs

A plugin for [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) that adds persistent NPCs with skins, holograms, and server transfer support.

## Features

- **Persistent NPCs** — NPCs survive server restarts, saved to `npcs.toml`
- **Auto Skin Fetching** — Skins are fetched from the Mojang API by username
- **Holograms** — Floating text lines above NPCs using invisible armor stands
- **Look at Nearest Player** — NPCs can track and face the nearest player
- **Server Transfer** — Clicking an NPC transfers the player to another server via [Gourd](https://github.com/Purdze/gourd) (a Pumpkin proxy)
- **Live Status Placeholders** — Hologram text supports `{status}`, `{online}`, and `{max}` placeholders that update in real time via Server List Ping
- **Hidden from Tab & Nametag** — NPCs don't appear in the player list and have no visible nametag

## Commands

All commands are under `/npc` and require the `npcs:npc` permission (OP level 2).

### NPC Management

| Command | Description |
|---|---|
| `/npc create <name>` | Create an NPC at your position. Skin is fetched from the Mojang API using `<name>` as the player username. |
| `/npc remove <id>` | Remove an NPC by its ID. |
| `/npc list` | List all NPCs with their IDs and positions. |
| `/npc looknear` | Toggle look-at-nearest-player for the NPC in your crosshair. |
| `/npc hologram add <text>` | Add a hologram line above the NPC in your crosshair. |

### Server Management

| Command | Description |
|---|---|
| `/npc server add <name> <address>` | Register a server (saved to `servers.toml`, starts status polling). |
| `/npc server remove <name>` | Unregister a server. |
| `/npc server list` | List all servers with their live status. |
| `/npc server set <name>` | Assign a registered server to the NPC in your crosshair. Players who click the NPC will be transferred via [Gourd](https://github.com/Purdze/Gourd). |

> Commands that target "the NPC in your crosshair" use a ~25 degree cone within 32 blocks.

## Configuration

The plugin stores its data in its data folder (typically `plugins/npcs/`).

### npcs.toml

Auto-managed by the plugin. Contains all NPC definitions. You generally don't need to edit this manually.

### servers.toml

Auto-managed by the plugin. Created automatically when you use `/npc server add`. Each entry maps a server name to its address:

```toml
[lobby]
address = "127.0.0.1:25566"

[survival]
address = "127.0.0.1:25567"
```

The plugin pings these servers every 5 seconds to update hologram placeholders.

### Status Placeholders

When an NPC has an assigned server, its hologram text can use these placeholders:

| Placeholder | Value |
|---|---|
| `{status}` | "Online" (green) or "Offline" (red) |
| `{online}` | Current player count |
| `{max}` | Max player count |

**Example:**

```
/npc server add lobby 127.0.0.1:25566
/npc create Lobby
/npc server set lobby
/npc hologram add Lobby
/npc hologram add {status} - {online}/{max}
```

## Server Transfers

Server transfer functionality requires [Gourd](https://github.com/Purdze/gourd), a proxy for Pumpkin. When a player clicks an NPC with an assigned server, the plugin sends a `gourd:transfer` plugin message. Gourd receives this and moves the player to the target backend server.

## Building

```sh
cargo build --release
```

The compiled plugin will be at `target/release/npcs.dll` (Windows) or `target/release/libnpcs.so` (Linux). Place it in your Pumpkin server's `plugins/` directory.
