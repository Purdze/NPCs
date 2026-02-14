use std::sync::atomic::{AtomicI32, Ordering};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod packets;
pub mod registry;
pub mod skin;

/// Entity IDs count down from -1000 to avoid collision with PumpkinMC's
/// CURRENT_ID which starts at 0 and increments.
static NEXT_ENTITY_ID: AtomicI32 = AtomicI32::new(-1000);

pub(crate) fn next_entity_id() -> i32 {
    NEXT_ENTITY_ID.fetch_sub(1, Ordering::Relaxed)
}

/// Deterministic UUIDv5 from "npc:{id}" so UUIDs are stable across restarts.
fn npc_uuid(id: u32) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_DNS, format!("npc:{id}").as_bytes())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpcLocation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpcSkin {
    pub textures: String,
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HologramLine {
    pub text: String,
    #[serde(skip)]
    pub entity_id: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Npc {
    pub id: u32,
    pub name: String,
    #[serde(skip)]
    pub uuid: Uuid,
    #[serde(skip)]
    pub entity_id: i32,
    pub location: NpcLocation,
    pub skin: Option<NpcSkin>,
    pub look_at_nearest: bool,
    pub holograms: Vec<HologramLine>,
    pub server: Option<String>,
}

impl Npc {
    pub fn new(id: u32, name: String, location: NpcLocation, skin: Option<NpcSkin>) -> Self {
        Self {
            uuid: npc_uuid(id),
            entity_id: next_entity_id(),
            id,
            name,
            location,
            skin,
            look_at_nearest: false,
            holograms: Vec::new(),
            server: None,
        }
    }

    pub fn init_runtime_fields(&mut self) {
        self.uuid = npc_uuid(self.id);
        self.entity_id = next_entity_id();
        for line in &mut self.holograms {
            line.entity_id = next_entity_id();
        }
    }
}
