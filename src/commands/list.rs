use pumpkin::command::args::ConsumedArgs;
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use crate::REGISTRY;

pub struct ListExecutor;

impl CommandExecutor for ListExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let registry = REGISTRY.get().expect("NPC registry not initialized");
            let npcs = registry.all().await;

            if npcs.is_empty() {
                sender
                    .send_message(TextComponent::text("No NPCs exist"))
                    .await;
                return Ok(0);
            }

            let mut msg = format!("NPCs ({}):\n", npcs.len());
            for npc in &npcs {
                msg.push_str(&format!(
                    "  #{} '{}' at ({:.1}, {:.1}, {:.1})\n",
                    npc.id, npc.name, npc.location.x, npc.location.y, npc.location.z
                ));
            }

            sender.send_message(TextComponent::text(msg)).await;

            Ok(npcs.len() as i32)
        })
    }
}
