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

| Command | Description |
|---|---|
| `/npc create <name>` | Create an NPC at your position. The skin is fetched from the Mojang API using `<name>` as the player username. |
| `/npc remove <id>` | Remove an NPC by its ID. |
| `/npc list` | List all NPCs with their IDs and positions. |
| `/npc looknear` | Toggle look-at-nearest-player for the NPC in your crosshair. |
| `/npc hologram add <text>` | Add a hologram line above the NPC in your crosshair. |
| `/npc server <server>` | Assign a server to the NPC in your crosshair. Players who click the NPC will be transferred to that server via [Gourd](https://github.com/Purdze/gourd). |

> Commands that target "the NPC in your crosshair" use a ~25 degree cone within 32 blocks.

## Configuration

The plugin stores its data in its data folder (typically `plugins/npcs/`).

### npcs.toml

Auto-managed by the plugin. Contains all NPC definitions. You generally don't need to edit this manually.

### servers.toml

Optional. Define servers for status placeholders and NPC transfers. Each entry maps a server name to its address:

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
| `{status}` | `§aOnline` or `§cOffline` |
| `{online}` | Current player count |
| `{max}` | Max player count |

**Example:**

```
/npc server lobby
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
