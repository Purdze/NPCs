use std::sync::Arc;

use bytes::BufMut;
use pumpkin::entity::player::Player;
use pumpkin::net::ClientPlatform;
use pumpkin_data::entity::EntityType;
use pumpkin_data::meta_data_type::MetaDataType;
use pumpkin_data::tracked_data::TrackedData;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::{
    CHeadRot, CPlayerInfoUpdate, CRemoveEntities, CRemovePlayerInfo, CSetEntityMetadata,
    CSpawnEntity, CUpdateEntityRot, Metadata, Player as ProtocolPlayer, PlayerAction,
    PlayerInfoFlags,
};
use pumpkin_util::math::vector3::Vector3;

use pumpkin_util::text::TextComponent;
use uuid::Uuid;

use pumpkin_protocol::ser::NetworkWriteExt;

use super::{HologramLine, Npc};

/// Serialize a TextComponent into a buffer using PumpkinMC's protocol serializer
/// (which encodes it as NBT via the "TextComponent" newtype struct detection).
fn write_text_component(
    buf: &mut Vec<u8>,
    tc: &TextComponent,
) -> Result<(), pumpkin_protocol::ser::WritingError> {
    use pumpkin_protocol::ser::serializer::Serializer as ProtocolSerializer;
    use serde::Serialize;
    let mut ser = ProtocolSerializer::new(buf);
    tc.serialize(&mut ser)
        .map_err(|e| pumpkin_protocol::ser::WritingError::Serde(e.to_string()))
}

/// Write a raw Set Player Team packet (mode 0 = create) to hide nametags.
/// We write raw bytes because PumpkinMC has no built-in team packet struct.
async fn send_team_nametag_hide(java: &pumpkin::net::java::JavaClient, npc: &Npc) -> bool {
    let version = java.version.load();
    let packet_id = pumpkin_data::packet::clientbound::PLAY_SET_PLAYER_TEAM.to_id(version);

    let mut buf: Vec<u8> = Vec::new();
    let empty_tc = TextComponent::text(String::new());

    let result: Result<(), pumpkin_protocol::ser::WritingError> = (|| {
        buf.write_var_int(&VarInt(packet_id))?;

        // Team Name
        let team_name = format!("npc_{}", npc.entity_id);
        buf.write_string(&team_name)?;

        // Mode = 0 (create team)
        buf.write_i8(0)?;

        // Display Name (Text Component as NBT)
        write_text_component(&mut buf, &empty_tc)?;

        // Friendly Flags
        buf.write_i8(0)?;

        // Name Tag Visibility (VarInt enum: 0=always, 1=never, 2=hideForOtherTeams, 3=hideForOwnTeam)
        buf.write_var_int(&VarInt(1))?;

        // Collision Rule (VarInt enum: 0=always, 1=never, 2=pushOtherTeams, 3=pushOwnTeam)
        buf.write_var_int(&VarInt(1))?;

        // Team Color = 21 (reset)
        buf.write_var_int(&VarInt(21))?;

        // Prefix (Text Component as NBT)
        write_text_component(&mut buf, &empty_tc)?;

        // Suffix (Text Component as NBT)
        write_text_component(&mut buf, &empty_tc)?;

        // Entities (1 member: the NPC's name)
        buf.write_var_int(&VarInt(1))?;
        buf.write_string(&npc.name)?;

        Ok(())
    })();

    match result {
        Ok(()) => {
            java.enqueue_packet_data(buf.into()).await;
            true
        }
        Err(e) => {
            log::error!("Failed to write team packet: {e:?}");
            false
        }
    }
}

/// Serialize and enqueue a packet, logging on failure. Returns false on error.
async fn send_packet<P: pumpkin_protocol::ClientPacket>(
    java: &pumpkin::net::java::JavaClient,
    packet: &P,
) -> bool {
    let mut buf = Vec::new();
    match java.write_packet(packet, &mut buf) {
        Ok(()) => {
            java.enqueue_packet_data(buf.into()).await;
            true
        }
        Err(e) => {
            log::error!("Failed to serialize packet: {e:?}");
            false
        }
    }
}

/// Send the full spawn packet sequence for an NPC to a single player.
pub async fn spawn_npc_for_player(npc: &Npc, player: &Arc<Player>) {
    let ClientPlatform::Java(java) = &player.client else {
        return;
    };

    let properties = if let Some(ref skin) = npc.skin {
        vec![pumpkin_protocol::Property {
            name: "textures".to_string(),
            value: skin.textures.clone(),
            signature: Some(skin.signature.clone()),
        }]
    } else {
        vec![]
    };

    // 1. CPlayerInfoUpdate - ADD_PLAYER | UPDATE_LISTED (listed = false to hide from tab)
    let actions = (PlayerInfoFlags::ADD_PLAYER | PlayerInfoFlags::UPDATE_LISTED).bits();
    let protocol_player = ProtocolPlayer {
        uuid: npc.uuid,
        actions: &[
            PlayerAction::AddPlayer {
                name: &npc.name,
                properties: &properties,
            },
            PlayerAction::UpdateListed(false),
        ],
    };
    {
        let players = [protocol_player];
        let packet = CPlayerInfoUpdate::new(actions, &players);
        if !send_packet(java, &packet).await {
            return;
        }
    }

    // 2. CSpawnEntity
    let position = Vector3::new(npc.location.x, npc.location.y, npc.location.z);
    let velocity = Vector3::new(0.0, 0.0, 0.0);
    let packet = CSpawnEntity::new(
        VarInt(npc.entity_id),
        npc.uuid,
        VarInt(i32::from(EntityType::PLAYER.id)),
        position,
        npc.location.pitch,
        npc.location.yaw,
        npc.location.yaw,
        VarInt(0),
        velocity,
    );
    if !send_packet(java, &packet).await {
        return;
    }

    // 3. CHeadRot
    let yaw_byte = (npc.location.yaw.rem_euclid(360.0) * 256.0 / 360.0).floor() as u8;
    if !send_packet(java, &CHeadRot::new(VarInt(npc.entity_id), yaw_byte)).await {
        return;
    }

    // 4. CSetEntityMetadata - skin layers 0x7F (all parts visible)
    {
        let version = java.version.load();
        let mut meta_buf = Vec::new();
        let meta = Metadata::new(
            TrackedData::DATA_PLAYER_MODE_CUSTOMIZATION_ID,
            MetaDataType::Byte,
            0x7Fu8,
        );
        if let Err(e) = meta.write(&mut meta_buf, &version) {
            log::error!("Failed to write NPC metadata: {e:?}");
            return;
        }
        meta_buf.put_u8(0xFF);
        let packet = CSetEntityMetadata::new(VarInt(npc.entity_id), meta_buf.into_boxed_slice());
        if !send_packet(java, &packet).await {
            return;
        }
    }

    // No CRemovePlayerInfo - UpdateListed(false) hides from tab while
    // keeping the skin textures in the client's player registry.

    // 5. CSetPlayerTeam - hide nametag by placing NPC in a team with nameTagVisibility "never"
    if !send_team_nametag_hide(java, npc).await {
        return;
    }

    // 6. Spawn hologram lines above the NPC
    spawn_holograms_for_player(npc, player).await;
}

/// Send rotation packets so the NPC faces a target position, for a single player.
pub async fn update_npc_look_at_player(npc: &Npc, player: &Arc<Player>, target_pos: &Vector3<f64>) {
    let ClientPlatform::Java(java) = &player.client else {
        return;
    };

    let dx = target_pos.x - npc.location.x;
    let dy = target_pos.y - npc.location.y; // feet-to-feet (eye offsets cancel)
    let dz = target_pos.z - npc.location.z;

    let horiz_dist = (dx * dx + dz * dz).sqrt();
    let yaw = (-dx).atan2(dz).to_degrees() as f32;
    let pitch = (-dy).atan2(horiz_dist).to_degrees() as f32;

    let yaw_byte = (yaw.rem_euclid(360.0) * 256.0 / 360.0).floor() as u8;
    let pitch_byte = (pitch.rem_euclid(360.0) * 256.0 / 360.0).floor() as u8;

    send_packet(java, &CHeadRot::new(VarInt(npc.entity_id), yaw_byte)).await;
    send_packet(
        java,
        &CUpdateEntityRot::new(VarInt(npc.entity_id), yaw_byte, pitch_byte, true),
    )
    .await;
}

/// Spawn all hologram armor stands above an NPC for a single player.
pub async fn spawn_holograms_for_player(npc: &Npc, player: &Arc<Player>) {
    let total = npc.holograms.len();
    for (i, line) in npc.holograms.iter().enumerate() {
        // Line 0 = topmost, line N = closest to head
        let y = npc.location.y + 2.05 + ((total - 1 - i) as f64 * 0.25);
        spawn_hologram_entity(npc, line, y, player).await;
    }
}

/// Despawn all hologram armor stands for an NPC from a single player.
pub async fn despawn_holograms_for_player(npc: &Npc, player: &Arc<Player>) {
    if npc.holograms.is_empty() {
        return;
    }
    let ClientPlatform::Java(java) = &player.client else {
        return;
    };
    let entity_ids: Vec<VarInt> = npc.holograms.iter().map(|h| VarInt(h.entity_id)).collect();
    send_packet(java, &CRemoveEntities::new(&entity_ids)).await;
}

/// Spawn a single invisible marker armor stand with a visible custom name.
async fn spawn_hologram_entity(npc: &Npc, line: &HologramLine, y: f64, player: &Arc<Player>) {
    let ClientPlatform::Java(java) = &player.client else {
        return;
    };

    let holo_uuid = Uuid::new_v5(
        &Uuid::NAMESPACE_DNS,
        format!("hologram:{}", line.entity_id).as_bytes(),
    );

    // Spawn armor stand entity
    let position = Vector3::new(npc.location.x, y, npc.location.z);
    let velocity = Vector3::new(0.0, 0.0, 0.0);
    let packet = CSpawnEntity::new(
        VarInt(line.entity_id),
        holo_uuid,
        VarInt(i32::from(EntityType::ARMOR_STAND.id)),
        position,
        0.0,
        0.0,
        0.0,
        VarInt(0),
        velocity,
    );
    if !send_packet(java, &packet).await {
        return;
    }

    // Metadata: invisible, custom name, name visible, no gravity, marker
    let version = java.version.load();
    let mut meta_buf = Vec::new();

    // Entity flags: invisible (bit 5 = 0x20)
    let flags = Metadata::new(TrackedData::DATA_FLAGS, MetaDataType::Byte, 0x20u8);
    if let Err(e) = flags.write(&mut meta_buf, &version) {
        log::error!("Failed to write hologram flags: {e:?}");
        return;
    }

    // Custom name (OptionalTextComponent) — resolve status placeholders if the NPC has a server
    let display_text = if let Some(ref server_name) = npc.server {
        crate::status::resolve_placeholders(&line.text, server_name)
    } else {
        line.text.clone()
    };
    let name = Metadata::new(
        TrackedData::DATA_CUSTOM_NAME,
        MetaDataType::OptionalTextComponent,
        Some(TextComponent::text(display_text)),
    );
    if let Err(e) = name.write(&mut meta_buf, &version) {
        log::error!("Failed to write hologram name: {e:?}");
        return;
    }

    // Custom name visible
    let visible = Metadata::new(TrackedData::DATA_NAME_VISIBLE, MetaDataType::Boolean, true);
    if let Err(e) = visible.write(&mut meta_buf, &version) {
        log::error!("Failed to write hologram visibility: {e:?}");
        return;
    }

    // No gravity
    let gravity = Metadata::new(TrackedData::DATA_NO_GRAVITY, MetaDataType::Boolean, true);
    if let Err(e) = gravity.write(&mut meta_buf, &version) {
        log::error!("Failed to write hologram gravity: {e:?}");
        return;
    }

    // Armor stand flags: marker (16)
    let stand_flags = Metadata::new(
        TrackedData::DATA_ARMOR_STAND_FLAGS,
        MetaDataType::Byte,
        16u8,
    );
    if let Err(e) = stand_flags.write(&mut meta_buf, &version) {
        log::error!("Failed to write hologram stand flags: {e:?}");
        return;
    }

    meta_buf.put_u8(0xFF); // terminator
    let packet = CSetEntityMetadata::new(VarInt(line.entity_id), meta_buf.into_boxed_slice());
    send_packet(java, &packet).await;
}

/// Send despawn packets for an NPC to a single player.
pub async fn despawn_npc_for_player(npc: &Npc, player: &Arc<Player>) {
    // Collect NPC entity + all hologram entities in one removal
    let mut entity_ids = vec![VarInt(npc.entity_id)];
    for line in &npc.holograms {
        entity_ids.push(VarInt(line.entity_id));
    }
    let ClientPlatform::Java(java) = &player.client else {
        return;
    };
    send_packet(java, &CRemoveEntities::new(&entity_ids)).await;
    send_packet(java, &CRemovePlayerInfo::new(&[npc.uuid])).await;
}
