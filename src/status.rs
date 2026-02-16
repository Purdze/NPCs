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
use std::sync::RwLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

use crate::{CONTEXT, DATA_FOLDER, REGISTRY};

const SERVERS_FILE: &str = "servers.toml";
const PING_TIMEOUT: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_secs(5);

static STATUS: OnceLock<Arc<RwLock<HashMap<String, ServerStatus>>>> = OnceLock::new();
static SERVERS: OnceLock<Arc<RwLock<HashMap<String, SocketAddr>>>> = OnceLock::new();

fn servers() -> Arc<RwLock<HashMap<String, SocketAddr>>> {
    SERVERS
        .get_or_init(|| Arc::new(RwLock::new(load_servers_config())))
        .clone()
}

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

async fn ping_server(addr: SocketAddr) -> Option<ServerStatus> {
    let mut stream = timeout(PING_TIMEOUT, TcpStream::connect(addr))
        .await
        .ok()?
        .ok()?;

    let mut handshake_data = Vec::new();
    write_varint(&mut handshake_data, 0x00);
    write_varint(&mut handshake_data, -1);
    let host = addr.ip().to_string();
    write_varint(&mut handshake_data, host.len() as i32);
    handshake_data.extend_from_slice(host.as_bytes());
    handshake_data.extend_from_slice(&addr.port().to_be_bytes());
    write_varint(&mut handshake_data, 1);

    let mut packet = Vec::new();
    write_varint(&mut packet, handshake_data.len() as i32);
    packet.extend_from_slice(&handshake_data);

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

    parse_slp_json(&json_str)
}

fn parse_slp_json(json: &str) -> Option<ServerStatus> {
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

pub fn resolve_placeholders(text: &str, server_name: &str) -> String {
    if !text.contains('{') {
        return text.to_string();
    }

    let status_map = match STATUS.get() {
        Some(s) => s,
        None => return text.to_string(),
    };

    let status = {
        let guard = status_map.read().unwrap();
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

pub fn has_placeholders(text: &str) -> bool {
    text.contains("{status}") || text.contains("{online}") || text.contains("{max}")
}

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

    meta_buf.put_u8(0xFF);

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

pub fn add_server(name: String, addr: SocketAddr) {
    let servers = servers();

    {
        let mut map = servers.write().unwrap();
        map.insert(name, addr);
        save_servers_config(&map);
    }

    // Ensure the status task is running
    start_status_task();
}

pub fn remove_server(name: &str) -> bool {
    let servers = servers();

    let mut map = servers.write().unwrap();
    let removed = map.remove(name).is_some();
    if removed {
        save_servers_config(&map);
    }
    removed
}

pub fn has_server(name: &str) -> bool {
    let servers = servers();

    let result = servers.read().unwrap().contains_key(name);
    result
}

pub fn list_servers() -> Vec<(String, SocketAddr, ServerStatus)> {
    let servers = servers();

    let server_map = servers.read().unwrap();
    let statuses = STATUS.get().map(|s| s.read().unwrap().clone());

    let mut result: Vec<_> = server_map
        .iter()
        .map(|(name, addr)| {
            let status = statuses
                .as_ref()
                .and_then(|s| s.get(name).cloned())
                .unwrap_or_default();
            (name.clone(), *addr, status)
        })
        .collect();
    result.sort_by(|(a, _, _), (b, _, _)| a.cmp(b));
    result
}

pub fn start_status_task() {
    let servers = servers();

    // Don't spawn if there are no servers yet
    if servers.read().unwrap().is_empty() {
        return;
    }

    // STATUS being already set means the task is already running
    let status_map = Arc::new(RwLock::new(HashMap::new()));
    if STATUS.set(Arc::clone(&status_map)).is_err() {
        return;
    }

    let server_count = servers.read().unwrap().len();
    log::info!("Status holograms enabled for {server_count} server(s)");

    tokio::spawn(async move {
        loop {
            let config: HashMap<String, SocketAddr> = servers.read().unwrap().clone();

            if !config.is_empty() {
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

                {
                    let mut map = status_map.write().unwrap();
                    *map = new_statuses;
                }

                push_hologram_updates().await;
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

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

fn save_servers_config(servers: &HashMap<String, SocketAddr>) {
    let path = DATA_FOLDER
        .get()
        .expect("Data folder not initialized")
        .join(SERVERS_FILE);

    let mut toml_str = String::new();
    let mut sorted: Vec<_> = servers.iter().collect();
    sorted.sort_by_key(|(name, _)| (*name).clone());
    for (name, addr) in sorted {
        toml_str.push_str(&format!("[{name}]\naddress = \"{addr}\"\n\n"));
    }

    if let Err(e) = std::fs::write(path, toml_str) {
        log::error!("Failed to write {SERVERS_FILE}: {e}");
    }
}

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
