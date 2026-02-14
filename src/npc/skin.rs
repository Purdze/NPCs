use serde::Deserialize;

use super::NpcSkin;

#[derive(Deserialize)]
struct MojangProfile {
    id: String,
}

#[derive(Deserialize)]
struct SessionProfile {
    properties: Vec<SessionProperty>,
}

#[derive(Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
    signature: Option<String>,
}

/// Fetch a player's skin from the Mojang API by username.
/// Returns None if the username doesn't exist or the API is unreachable.
/// This is a blocking HTTP call.
pub fn fetch_skin(username: &str) -> Option<NpcSkin> {
    // Step 1: Username -> UUID
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{username}");
    let profile: MojangProfile = ureq::get(&url).call().ok()?.into_json().ok()?;

    // Step 2: UUID -> session profile with signed textures
    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}?unsigned=false",
        profile.id
    );
    let session: SessionProfile = ureq::get(&url).call().ok()?.into_json().ok()?;

    // Find the "textures" property
    let textures = session
        .properties
        .into_iter()
        .find(|p| p.name == "textures")?;

    Some(NpcSkin {
        textures: textures.value,
        signature: textures.signature?,
    })
}
