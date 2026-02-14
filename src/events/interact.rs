use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use pumpkin::plugin::api::events::player::player_interact_unknown_entity_event::PlayerInteractUnknownEntityEvent;
use pumpkin::plugin::EventHandler;
use pumpkin::server::Server;
use pumpkin_protocol::java::server::play::ActionType;

use crate::REGISTRY;

pub struct InteractHandler;

impl EventHandler<PlayerInteractUnknownEntityEvent> for InteractHandler {
    fn handle<'a>(
        &'a self,
        _server: &'a Arc<Server>,
        event: &'a PlayerInteractUnknownEntityEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if event.action != ActionType::Interact {
                return;
            }

            let registry = match REGISTRY.get() {
                Some(r) => r,
                None => return,
            };

            let npc = match registry.get_by_entity_id(event.entity_id).await {
                Some(n) => n,
                None => return,
            };

            let server_name = match &npc.server {
                Some(s) => s,
                None => return,
            };

            log::info!(
                "Sending gourd:transfer for '{}' -> '{}'",
                event.player.gameprofile.name,
                server_name
            );
            event
                .player
                .send_custom_payload("gourd:transfer", server_name.as_bytes())
                .await;
        })
    }
}
