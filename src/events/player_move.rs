use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use pumpkin::plugin::api::events::player::player_move::PlayerMoveEvent;
use pumpkin::plugin::EventHandler;
use pumpkin::server::Server;

use crate::REGISTRY;

pub struct MoveHandler;

impl EventHandler<PlayerMoveEvent> for MoveHandler {
    fn handle<'a>(
        &'a self,
        _server: &'a Arc<Server>,
        event: &'a PlayerMoveEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let registry = match REGISTRY.get() {
                Some(r) => r,
                None => return,
            };

            let npcs = registry.look_at_nearest_npcs().await;
            if npcs.is_empty() {
                return;
            }

            for npc in &npcs {
                let dx = event.to.x - npc.location.x;
                let dz = event.to.z - npc.location.z;
                let dist_sq = dx * dx + dz * dz;

                // Only update if player is within 32 blocks
                if dist_sq > 32.0 * 32.0 {
                    continue;
                }

                crate::npc::packets::update_npc_look_at_player(npc, &event.player, &event.to).await;
            }
        })
    }
}
