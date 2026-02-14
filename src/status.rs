use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use bytes::BufMut;
use pumpkin::entity::player::Player;
use pumpkin::net::ClientPlatform;
use pumpkin_data::meta_data_type::MetaDataType;
use pumpkin_data::tracked_data::TrackedData;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::{CSetEntityMetadata, Metadata};
use pumpkin_util::text::TextComponent;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

use crate::{CONTEXT, DATA_FOLDER, REGISTRY};

const SERVERS_FILE: &str = "servers.toml";
const PING_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_secs(5);

static STATUS: OnceLock<Arc<RwLock<HashMap<String, ServerStatus>>>> = OnceLock::new();

#[derive(Clone, Debug, Default)]
pub struct ServerStatus {
    pub online: bool,
    pub players_online: u32,
    pub players_max: u32,
}

#[derive(Deserialize)]
struct ServersConfig {
    #[serde(flatten)]
    servers: HashMap<String, ServerEntry>,
}

#[derive(Deserialize)]
struct ServerEntry {
    address: String,
}

/// Write a VarInt to a buffer and return how many bytes were written.
fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value = ((value as u32) >> 7) as i32;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Read a VarInt from an async reader.
async fn read_varint<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> std::io::Result<i32> {
    let mut result: i32 = 0;
    let mut shift = 0;
    loop {
        let byte = reader.read_u8().await?;
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 32 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "VarInt too big",
            ));
        }
    }
    Ok(result)
}

/// Ping a Minecraft server using the Server List Ping protocol.
/// Returns `None` on connection failure or protocol error.
async fn ping_server(addr: SocketAddr) -> Option<ServerStatus> {
    let mut stream = timeout(PING_TIMEOUT, TcpStream::connect(addr))
        .await
        .ok()?
        .ok()?;

    // Build Handshake packet (id=0x00)
    let mut handshake_data = Vec::new();
    write_varint(&mut handshake_data, 0x00); // packet id
    write_varint(&mut handshake_data, -1); // protocol version (-1 = ping)
                                           // server address as string
    let host = addr.ip().to_string();
    write_varint(&mut handshake_data, host.len() as i32);
    handshake_data.extend_from_slice(host.as_bytes());
    handshake_data.extend_from_slice(&addr.port().to_be_bytes());
    write_varint(&mut handshake_data, 1); // next state = Status

    // Send handshake: length-prefixed
    let mut packet = Vec::new();
    write_varint(&mut packet, handshake_data.len() as i32);
    packet.extend_from_slice(&handshake_data);

    // Status Request packet (id=0x00, no fields)
    let mut status_req = Vec::new();
    write_varint(&mut status_req, 0x00);
    let mut status_packet = Vec::new();
    write_varint(&mut status_packet, status_req.len() as i32);
    status_packet.extend_from_slice(&status_req);

    packet.extend_from_slice(&status_packet);

    timeout(PING_TIMEOUT, stream.write_all(&packet))
        .await
        .ok()?
        .ok()?;

    // Read Status Response
    let _length = timeout(PING_TIMEOUT, read_varint(&mut stream))
        .await
        .ok()?
        .ok()?;
    let packet_id = timeout(PING_TIMEOUT, read_varint(&mut stream))
        .await
        .ok()?
        .ok()?;
    if packet_id != 0x00 {
        return None;
    }

    let json_len = timeout(PING_TIMEOUT, read_varint(&mut stream))
        .await
        .ok()?
        .ok()?;
    if json_len <= 0 || json_len > 1_000_000 {
        return None;
    }

    let mut json_buf = vec![0u8; json_len as usize];
    timeout(PING_TIMEOUT, stream.read_exact(&mut json_buf))
        .await
        .ok()?
        .ok()?;

    let json_str = String::from_utf8(json_buf).ok()?;

    // Parse the JSON to extract player counts
    // Format: {"players":{"max":20,"online":3,...},...}
    parse_slp_json(&json_str)
}

/// Parse the SLP JSON response to extract player counts.
fn parse_slp_json(json: &str) -> Option<ServerStatus> {
    // Minimal JSON parsing — look for "players" object with "online" and "max"
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let players = v.get("players")?;
    let online = players.get("online")?.as_u64()? as u32;
    let max = players.get("max")?.as_u64()? as u32;

    Some(ServerStatus {
        online: true,
        players_online: online,
        players_max: max,
    })
}

/// Replace placeholder tokens in hologram text with live server status.
pub fn resolve_placeholders(text: &str, server_name: &str) -> String {
    if !text.contains('{') {
        return text.to_string();
    }

    let status_map = match STATUS.get() {
        Some(s) => s,
        None => return text.to_string(),
    };

    let status = {
        let guard = status_map.blocking_read();
        guard.get(server_name).cloned().unwrap_or_default()
    };

    let (status_str, online_str, max_str) = if status.online {
        (
            "\u{00A7}aOnline".to_string(),
            status.players_online.to_string(),
            status.players_max.to_string(),
        )
    } else {
        (
            "\u{00A7}cOffline".to_string(),
            "0".to_string(),
            "0".to_string(),
        )
    };

    text.replace("{status}", &status_str)
        .replace("{online}", &online_str)
        .replace("{max}", &max_str)
}

/// Returns true if the text contains any server status placeholders.
pub fn has_placeholders(text: &str) -> bool {
    text.contains("{status}") || text.contains("{online}") || text.contains("{max}")
}

/// Send a metadata update to change a hologram's custom name for a single player.
async fn update_hologram_text(entity_id: i32, text: &str, player: &Arc<Player>) {
    let ClientPlatform::Java(java) = &player.client else {
        return;
    };

    let version = java.version.load();
    let mut meta_buf = Vec::new();

    let name = Metadata::new(
        TrackedData::DATA_CUSTOM_NAME,
        MetaDataType::OptionalTextComponent,
        Some(TextComponent::text(text.to_string())),
    );
    if let Err(e) = name.write(&mut meta_buf, &version) {
        log::error!("Failed to write hologram name update: {e:?}");
        return;
    }

    meta_buf.put_u8(0xFF); // terminator

    let packet = CSetEntityMetadata::new(VarInt(entity_id), meta_buf.into_boxed_slice());

    let mut buf = Vec::new();
    match java.write_packet(&packet, &mut buf) {
        Ok(()) => {
            java.enqueue_packet_data(buf.into()).await;
        }
        Err(e) => {
            log::error!("Failed to serialize hologram update packet: {e:?}");
        }
    }
}

/// Start the background task that periodically pings servers and updates holograms.
pub fn start_status_task() {
    let config = load_servers_config();
    if config.is_empty() {
        log::info!("No servers.toml found or no servers configured — status holograms disabled");
        return;
    }

    let status_map = Arc::new(RwLock::new(HashMap::new()));
    STATUS.set(Arc::clone(&status_map)).ok();

    log::info!(
        "Status holograms enabled for {} server(s): {}",
        config.len(),
        config.keys().cloned().collect::<Vec<_>>().join(", ")
    );

    tokio::spawn(async move {
        loop {
            // Ping all servers concurrently
            let mut handles = Vec::new();
            for (name, addr) in &config {
                let name = name.clone();
                let addr = *addr;
                handles.push(tokio::spawn(async move {
                    let status = ping_server(addr).await.unwrap_or_default();
                    (name, status)
                }));
            }

            let mut new_statuses = HashMap::new();
            for handle in handles {
                if let Ok((name, status)) = handle.await {
                    new_statuses.insert(name, status);
                }
            }

            // Update the shared status map
            {
                let mut map = status_map.write().await;
                *map = new_statuses;
            }

            // Push hologram updates to all online players
            push_hologram_updates().await;

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

/// Push hologram text updates to all online players for NPCs that have placeholders.
async fn push_hologram_updates() {
    let context = match CONTEXT.get() {
        Some(c) => c,
        None => return,
    };

    let registry = match REGISTRY.get() {
        Some(r) => r,
        None => return,
    };

    let npcs = registry.all().await;
    let players = context.server.get_all_players();

    if players.is_empty() {
        return;
    }

    for npc in &npcs {
        let server_name = match &npc.server {
            Some(s) => s,
            None => continue,
        };

        for line in &npc.holograms {
            if !has_placeholders(&line.text) {
                continue;
            }

            let resolved = resolve_placeholders(&line.text, server_name);

            for player in &players {
                update_hologram_text(line.entity_id, &resolved, player).await;
            }
        }
    }
}

/// Load the servers.toml config file, returning a map of server name -> socket address.
fn load_servers_config() -> HashMap<String, SocketAddr> {
    let path = DATA_FOLDER
        .get()
        .expect("Data folder not initialized")
        .join(SERVERS_FILE);
    if !path.exists() {
        return HashMap::new();
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to read {SERVERS_FILE}: {e}");
            return HashMap::new();
        }
    };

    let config: ServersConfig = match toml::from_str(&contents) {
        Ok(c) => c,
        Err(e) => {
            log::error!("Failed to parse {SERVERS_FILE}: {e}");
            return HashMap::new();
        }
    };

    let mut result = HashMap::new();
    for (name, entry) in config.servers {
        match entry.address.parse::<SocketAddr>() {
            Ok(addr) => {
                result.insert(name, addr);
            }
            Err(e) => {
                log::error!("Invalid address for server '{name}': {e}");
            }
        }
    }
    result
}
