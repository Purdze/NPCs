use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use pumpkin::plugin::api::events::player::player_join::PlayerJoinEvent;
use pumpkin::plugin::EventHandler;
use pumpkin::server::Server;

use crate::REGISTRY;

pub struct JoinHandler;

impl EventHandler<PlayerJoinEvent> for JoinHandler {
    fn handle<'a>(
        &'a self,
        _server: &'a Arc<Server>,
        event: &'a PlayerJoinEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let registry = match REGISTRY.get() {
                Some(r) => r,
                None => return,
            };

            let npcs = registry.all().await;
            for npc in &npcs {
                crate::npc::packets::spawn_npc_for_player(npc, &event.player).await;
            }
        })
    }
}
