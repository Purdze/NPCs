use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{HologramLine, Npc, NpcLocation, NpcSkin};
use crate::DATA_FOLDER;

const NPCS_FILE: &str = "npcs.toml";

fn npcs_path() -> PathBuf {
    DATA_FOLDER
        .get()
        .expect("Data folder not initialized")
        .join(NPCS_FILE)
}

#[derive(Serialize, Deserialize)]
struct NpcConfig {
    #[serde(default)]
    npcs: Vec<Npc>,
}

#[allow(dead_code)]
pub struct NpcRegistry {
    npcs: RwLock<HashMap<u32, Npc>>,
    next_id: AtomicU32,
    selected: RwLock<HashMap<Uuid, u32>>,
}

#[allow(dead_code)]
impl NpcRegistry {
    pub fn new() -> Self {
        Self {
            npcs: RwLock::new(HashMap::new()),
            next_id: AtomicU32::new(1),
            selected: RwLock::new(HashMap::new()),
        }
    }

    pub async fn load(&self) -> usize {
        let path = npcs_path();
        if !path.exists() {
            return 0;
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to read {NPCS_FILE}: {e}");
                return 0;
            }
        };
        let mut config: NpcConfig = match toml::from_str(&contents) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to parse {NPCS_FILE}: {e}");
                return 0;
            }
        };

        let mut max_id = 0u32;
        for npc in &mut config.npcs {
            npc.init_runtime_fields();
            if npc.id > max_id {
                max_id = npc.id;
            }
        }

        let count = config.npcs.len();
        let mut npcs = self.npcs.write().await;
        for npc in config.npcs {
            npcs.insert(npc.id, npc);
        }
        self.next_id.store(max_id + 1, Ordering::Relaxed);
        count
    }

    async fn save(&self) {
        let npcs = self.npcs.read().await;
        let mut sorted: Vec<&Npc> = npcs.values().collect();
        sorted.sort_by_key(|n| n.id);

        let config = NpcConfig {
            npcs: sorted.into_iter().cloned().collect(),
        };

        let contents = match toml::to_string_pretty(&config) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to serialize NPCs: {e}");
                return;
            }
        };

        if let Err(e) = std::fs::write(npcs_path(), contents) {
            log::error!("Failed to write {NPCS_FILE}: {e}");
        }
    }

    pub async fn create(&self, name: String, location: NpcLocation, skin: Option<NpcSkin>) -> Npc {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let npc = Npc::new(id, name, location, skin);
        self.npcs.write().await.insert(id, npc.clone());
        self.save().await;
        npc
    }

    pub async fn remove(&self, id: u32) -> Option<Npc> {
        let npc = self.npcs.write().await.remove(&id);
        if npc.is_some() {
            self.save().await;
        }
        npc
    }

    pub async fn get(&self, id: u32) -> Option<Npc> {
        self.npcs.read().await.get(&id).cloned()
    }

    pub async fn get_by_entity_id(&self, entity_id: i32) -> Option<Npc> {
        self.npcs
            .read()
            .await
            .values()
            .find(|n| n.entity_id == entity_id)
            .cloned()
    }

    pub async fn all(&self) -> Vec<Npc> {
        self.npcs.read().await.values().cloned().collect()
    }

    pub async fn select(&self, player_uuid: Uuid, npc_id: u32) {
        self.selected.write().await.insert(player_uuid, npc_id);
    }

    pub async fn get_selected(&self, player_uuid: &Uuid) -> Option<u32> {
        self.selected.read().await.get(player_uuid).copied()
    }

    async fn modify<F, R>(&self, id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut Npc) -> R,
    {
        let mut npcs = self.npcs.write().await;
        let npc = npcs.get_mut(&id)?;
        let result = f(npc);
        drop(npcs);
        self.save().await;
        Some(result)
    }

    pub async fn toggle_look_at_nearest(&self, id: u32) -> Option<bool> {
        self.modify(id, |npc| {
            npc.look_at_nearest = !npc.look_at_nearest;
            npc.look_at_nearest
        })
        .await
    }

    pub async fn add_hologram(&self, id: u32, text: String) -> Option<Npc> {
        self.modify(id, |npc| {
            npc.holograms.push(HologramLine {
                text,
                entity_id: super::next_entity_id(),
            });
            npc.clone()
        })
        .await
    }

    pub async fn set_server(&self, id: u32, server: Option<String>) -> Option<Npc> {
        self.modify(id, |npc| {
            npc.server = server;
            npc.clone()
        })
        .await
    }

    pub async fn look_at_nearest_npcs(&self) -> Vec<Npc> {
        self.npcs
            .read()
            .await
            .values()
            .filter(|n| n.look_at_nearest)
            .cloned()
            .collect()
    }
}
