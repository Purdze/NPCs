use std::net::SocketAddr;

use pumpkin::command::args::simple::SimpleArgConsumer;
use pumpkin::command::args::{ConsumedArgs, FindArg};
use pumpkin::command::dispatcher::CommandError;
use pumpkin::command::{CommandExecutor, CommandResult, CommandSender};
use pumpkin::server::Server;
use pumpkin_util::text::TextComponent;

use super::find_npc_in_crosshair;
use crate::REGISTRY;

pub struct ServerAddExecutor;

impl CommandExecutor for ServerAddExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let name = SimpleArgConsumer::find_arg(args, "name")
                .map_err(|_| CommandError::InvalidConsumption(Some("name".into())))?
                .to_string();

            let address_str = SimpleArgConsumer::find_arg(args, "address")
                .map_err(|_| CommandError::InvalidConsumption(Some("address".into())))?;

            let addr: SocketAddr = address_str.parse().map_err(|_| {
                CommandError::CommandFailed(TextComponent::text(format!(
                    "Invalid address: {address_str} (expected ip:port)"
                )))
            })?;

            crate::status::add_server(name.clone(), addr);

            sender
                .send_message(TextComponent::text(format!(
                    "Registered server '{name}' ({addr})"
                )))
                .await;

            Ok(1)
        })
    }
}

pub struct ServerRemoveExecutor;

impl CommandExecutor for ServerRemoveExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let name = SimpleArgConsumer::find_arg(args, "name")
                .map_err(|_| CommandError::InvalidConsumption(Some("name".into())))?;

            if crate::status::remove_server(name) {
                sender
                    .send_message(TextComponent::text(format!("Removed server '{name}'")))
                    .await;
                Ok(1)
            } else {
                sender
                    .send_message(TextComponent::text(format!(
                        "No server found with name '{name}'"
                    )))
                    .await;
                Ok(0)
            }
        })
    }
}

pub struct ServerListExecutor;

impl CommandExecutor for ServerListExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let servers = crate::status::list_servers();

            if servers.is_empty() {
                sender
                    .send_message(TextComponent::text("No servers registered"))
                    .await;
                return Ok(0);
            }

            let mut msg = format!("Servers ({}):\n", servers.len());
            for (name, addr, status) in &servers {
                let state = if status.online {
                    format!("Online {}/{}", status.players_online, status.players_max)
                } else {
                    "Offline".to_string()
                };
                msg.push_str(&format!("  {name} ({addr}) — {state}\n"));
            }

            sender.send_message(TextComponent::text(msg)).await;

            Ok(servers.len() as i32)
        })
    }
}

pub struct ServerSetExecutor;

impl CommandExecutor for ServerSetExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let server_name = SimpleArgConsumer::find_arg(args, "name")
                .map_err(|_| CommandError::InvalidConsumption(Some("name".into())))?
                .to_string();

            if !crate::status::has_server(&server_name) {
                sender
                    .send_message(TextComponent::text(format!(
                        "Unknown server '{server_name}'. Use /npc server add <name> <address> first"
                    )))
                    .await;
                return Ok(0);
            }

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
                    "Set server '{server_name}' on NPC '{}' (ID {})",
                    npc.name, npc.id
                )))
                .await;

            Ok(1)
        })
    }
}
