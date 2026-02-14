#![allow(clippy::async_yields_async)]

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use pumpkin::plugin::api::events::EventPriority;
use pumpkin::plugin::Context;
use pumpkin_api_macros::{plugin_impl, plugin_method};
use pumpkin_util::permission::{Permission, PermissionDefault, PermissionLvl};

mod commands;
mod events;
mod npc;
mod status;

use npc::registry::NpcRegistry;

pub(crate) static REGISTRY: OnceLock<Arc<NpcRegistry>> = OnceLock::new();
pub(crate) static CONTEXT: OnceLock<Arc<Context>> = OnceLock::new();
pub(crate) static DATA_FOLDER: OnceLock<PathBuf> = OnceLock::new();

#[plugin_method]
async fn on_load(&mut self, server: Arc<Context>) -> Result<(), String> {
    server.init_log();

    let data_folder = server.get_data_folder();
    std::fs::create_dir_all(&data_folder)
        .map_err(|e| format!("Failed to create data folder: {e}"))?;
    DATA_FOLDER
        .set(data_folder)
        .map_err(|_| "Data folder already initialized")?;

    let registry = Arc::new(NpcRegistry::new());
    let loaded = registry.load().await;
    if loaded > 0 {
        log::info!("Loaded {loaded} NPC(s) from npcs.toml");
    }
    REGISTRY
        .set(registry)
        .map_err(|_| "Registry already initialized")?;
    CONTEXT
        .set(Arc::clone(&server))
        .map_err(|_| "Context already initialized")?;

    server
        .register_permission(Permission::new(
            "npcs:npc",
            "Manage NPCs",
            PermissionDefault::Op(PermissionLvl::Two),
        ))
        .await
        .map_err(|e| format!("Failed to register permission: {e}"))?;

    let tree = commands::build_npc_command();
    server.register_command(tree, "npcs:npc").await;

    server
        .register_event(
            Arc::new(events::join::JoinHandler),
            EventPriority::Normal,
            false,
        )
        .await;
    server
        .register_event(
            Arc::new(events::player_move::MoveHandler),
            EventPriority::Normal,
            false,
        )
        .await;
    server
        .register_event(
            Arc::new(events::interact::InteractHandler),
            EventPriority::Normal,
            false,
        )
        .await;

    status::start_status_task();

    log::info!("NPCs plugin loaded — /npc command available");

    Ok(())
}

#[plugin_impl]
pub struct Npcs {}

impl Npcs {
    pub fn new() -> Self {
        Npcs {}
    }
}

impl Default for Npcs {
    fn default() -> Self {
        Self::new()
    }
}
