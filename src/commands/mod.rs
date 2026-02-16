pub mod create;
pub mod hologram;
pub mod list;
pub mod looknear;
pub mod remove;
pub mod server;

use pumpkin::command::args::message::MsgArgConsumer;
use pumpkin::command::args::simple::SimpleArgConsumer;
use pumpkin::command::tree::builder::{argument, literal};
use pumpkin::command::tree::CommandTree;
use pumpkin::entity::player::Player;

use crate::npc::Npc;

pub fn find_npc_in_crosshair(player: &Player, npcs: &[Npc]) -> Option<u32> {
    let pos = player.living_entity.entity.pos.load();
    let yaw = player.living_entity.entity.yaw.load();
    let pitch = player.living_entity.entity.pitch.load();

    let yaw_rad = (yaw as f64).to_radians();
    let pitch_rad = (pitch as f64).to_radians();
    let look_x = -yaw_rad.sin() * pitch_rad.cos();
    let look_y = -pitch_rad.sin();
    let look_z = yaw_rad.cos() * pitch_rad.cos();

    let eye_y = pos.y + 1.62;

    let mut best: Option<(u32, f64)> = None;

    for npc in npcs {
        let dx = npc.location.x - pos.x;
        let dy = (npc.location.y + 0.9) - eye_y;
        let dz = npc.location.z - pos.z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if !(0.1..=32.0).contains(&dist) {
            continue;
        }

        let nx = dx / dist;
        let ny = dy / dist;
        let nz = dz / dist;
        let dot = look_x * nx + look_y * ny + look_z * nz;

        if dot > 0.9 && best.is_none_or(|(_, d)| dot > d) {
            best = Some((npc.id, dot));
        }
    }

    best.map(|(id, _)| id)
}

pub fn build_npc_command() -> CommandTree {
    CommandTree::new(["npc"], "Manage NPCs")
        .then(
            literal("create")
                .then(argument("name", SimpleArgConsumer).execute(create::CreateExecutor)),
        )
        .then(
            literal("remove")
                .then(argument("id", SimpleArgConsumer).execute(remove::RemoveExecutor)),
        )
        .then(literal("list").execute(list::ListExecutor))
        .then(literal("looknear").execute(looknear::LookNearExecutor))
        .then(
            literal("server")
                .then(
                    literal("add").then(argument("name", SimpleArgConsumer).then(
                        argument("address", SimpleArgConsumer).execute(server::ServerAddExecutor),
                    )),
                )
                .then(literal("remove").then(
                    argument("name", SimpleArgConsumer).execute(server::ServerRemoveExecutor),
                ))
                .then(literal("list").execute(server::ServerListExecutor))
                .then(
                    literal("set").then(
                        argument("name", SimpleArgConsumer).execute(server::ServerSetExecutor),
                    ),
                ),
        )
        .then(
            literal("hologram").then(
                literal("add")
                    .then(argument("text", MsgArgConsumer).execute(hologram::HologramAddExecutor)),
            ),
        )
}
