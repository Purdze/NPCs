use pumpkin::command::args::ConsumedArgs;
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use super::find_npc_in_crosshair;
use crate::REGISTRY;

pub struct LookNearExecutor;

impl CommandExecutor for LookNearExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let CommandSender::Player(player) = sender else {
                sender
                    .send_message(TextComponent::text("Only players can use this command"))
                    .await;
                return Ok(0);
            };

            let registry = REGISTRY.get().expect("NPC registry not initialized");
            let npcs = registry.all().await;

            if npcs.is_empty() {
                sender
                    .send_message(TextComponent::text("No NPCs exist"))
                    .await;
                return Ok(0);
            }

            let Some(npc_id) = find_npc_in_crosshair(player, &npcs) else {
                sender
                    .send_message(TextComponent::text("No NPC found in crosshair"))
                    .await;
                return Ok(0);
            };

            let Some(new_state) = registry.toggle_look_at_nearest(npc_id).await else {
                sender
                    .send_message(TextComponent::text("NPC not found"))
                    .await;
                return Ok(0);
            };

            let npc = registry.get(npc_id).await.expect("NPC just toggled");
            let state_msg = if new_state { "enabled" } else { "disabled" };
            sender
                .send_message(TextComponent::text(format!(
                    "Look-at-nearest {state_msg} for NPC '{}' (ID {})",
                    npc.name, npc.id
                )))
                .await;

            Ok(1)
        })
    }
}
