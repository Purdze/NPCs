use pumpkin::command::args::message::MsgArgConsumer;
use pumpkin::command::args::{ConsumedArgs, FindArg};
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use super::find_npc_in_crosshair;
use crate::REGISTRY;

pub struct HologramAddExecutor;

impl CommandExecutor for HologramAddExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let text = MsgArgConsumer::find_arg(args, "text")
                .map_err(|_| {
                    pumpkin::command::dispatcher::CommandError::InvalidConsumption(Some(
                        "text".into(),
                    ))
                })?
                .to_string();

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

            let Some(npc_before) = registry.get(npc_id).await else {
                sender
                    .send_message(TextComponent::text("NPC not found"))
                    .await;
                return Ok(0);
            };

            for p in server.get_all_players() {
                crate::npc::packets::despawn_holograms_for_player(&npc_before, &p).await;
            }

            let Some(npc_after) = registry.add_hologram(npc_id, text.clone()).await else {
                sender
                    .send_message(TextComponent::text("NPC not found"))
                    .await;
                return Ok(0);
            };

            for p in server.get_all_players() {
                crate::npc::packets::spawn_holograms_for_player(&npc_after, &p).await;
            }

            sender
                .send_message(TextComponent::text(format!(
                    "Added hologram '{}' to NPC '{}' (ID {})",
                    text, npc_after.name, npc_after.id
                )))
                .await;

            Ok(1)
        })
    }
}
