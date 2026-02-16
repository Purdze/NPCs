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

pub fn fetch_skin(username: &str) -> Option<NpcSkin> {
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{username}");
    let profile: MojangProfile = ureq::get(&url).call().ok()?.into_json().ok()?;

    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}?unsigned=false",
        profile.id
    );
    let session: SessionProfile = ureq::get(&url).call().ok()?.into_json().ok()?;

    let textures = session
        .properties
        .into_iter()
        .find(|p| p.name == "textures")?;

    Some(NpcSkin {
        textures: textures.value,
        signature: textures.signature?,
    })
}
