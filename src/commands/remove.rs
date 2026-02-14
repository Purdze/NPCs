use pumpkin::command::args::simple::SimpleArgConsumer;
use pumpkin::command::args::{ConsumedArgs, FindArg};
use pumpkin::command::dispatcher::CommandError;
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use crate::REGISTRY;

pub struct RemoveExecutor;

impl CommandExecutor for RemoveExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let id_str = SimpleArgConsumer::find_arg(args, "id")
                .map_err(|_| CommandError::InvalidConsumption(Some("id".into())))?;

            let id: u32 = id_str.parse().map_err(|_| {
                CommandError::CommandFailed(TextComponent::text(format!(
                    "Invalid NPC ID: {id_str}"
                )))
            })?;

            let registry = REGISTRY.get().expect("NPC registry not initialized");

            let Some(npc) = registry.remove(id).await else {
                sender
                    .send_message(TextComponent::text(format!("No NPC found with ID {id}")))
                    .await;
                return Ok(0);
            };

            for p in server.get_all_players() {
                crate::npc::packets::despawn_npc_for_player(&npc, &p).await;
            }

            sender
                .send_message(TextComponent::text(format!(
                    "Removed NPC '{}' (ID {})",
                    npc.name, npc.id
                )))
                .await;

            Ok(1)
        })
    }
}
