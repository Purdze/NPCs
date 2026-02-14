use pumpkin::command::args::simple::SimpleArgConsumer;
use pumpkin::command::args::{ConsumedArgs, FindArg};
use pumpkin::command::dispatcher::CommandError;
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use crate::npc::skin;
use crate::npc::NpcLocation;
use crate::REGISTRY;

pub struct CreateExecutor;

impl CommandExecutor for CreateExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let name = SimpleArgConsumer::find_arg(args, "name")
                .map_err(|_| CommandError::InvalidConsumption(Some("name".into())))?;

            let CommandSender::Player(player) = sender else {
                sender
                    .send_message(TextComponent::text("Only players can create NPCs"))
                    .await;
                return Ok(0);
            };

            let registry = REGISTRY.get().expect("NPC registry not initialized");

            // Fetch skin from Mojang API (blocking, typically <200ms)
            let fetched_skin = skin::fetch_skin(name);
            let has_skin = fetched_skin.is_some();

            let pos = player.living_entity.entity.pos.load();
            let yaw = player.living_entity.entity.yaw.load();
            let pitch = player.living_entity.entity.pitch.load();

            let location = NpcLocation {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                yaw,
                pitch,
            };

            let npc = registry
                .create(name.to_string(), location, fetched_skin)
                .await;

            for p in server.get_all_players() {
                crate::npc::packets::spawn_npc_for_player(&npc, &p).await;
            }

            let skin_msg = if has_skin {
                " with skin"
            } else {
                " (no skin found)"
            };
            sender
                .send_message(TextComponent::text(format!(
                    "Created NPC '{}' (ID {}){skin_msg}",
                    npc.name, npc.id
                )))
                .await;

            Ok(1)
        })
    }
}
