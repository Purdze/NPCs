use pumpkin::command::args::simple::SimpleArgConsumer;
use pumpkin::command::args::{ConsumedArgs, FindArg};
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use super::find_npc_in_crosshair;
use crate::REGISTRY;

pub struct ServerExecutor;

impl CommandExecutor for ServerExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let server_name = SimpleArgConsumer::find_arg(args, "server")
                .map_err(|_| {
                    pumpkin::command::dispatcher::CommandError::InvalidConsumption(Some(
                        "server".into(),
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

            let Some(npc) = registry.set_server(npc_id, Some(server_name.clone())).await else {
                sender
                    .send_message(TextComponent::text("NPC not found"))
                    .await;
                return Ok(0);
            };

            sender
                .send_message(TextComponent::text(format!(
                    "Set server '{}' on NPC '{}' (ID {})",
                    server_name, npc.name, npc.id
                )))
                .await;

            Ok(1)
        })
    }
}
