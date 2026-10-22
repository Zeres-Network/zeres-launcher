//! Presencia en Discord mientras se juega (de momento solo en el canal dev): sale "Jugando a Zeres Launcher" (el nombre
//! de la aplicacion de Discord) con la instancia debajo: "Cobblemon" o "Zeres Network". Si Discord no esta abierto o no
//! hay id de aplicacion, no hace nada.
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

/// Id de la aplicacion "Zeres Launcher" del Discord Developer Portal (no es secreto). Vacio = sin presencia.
pub const APP_ID: &str = "1562598911103729724";

pub struct Presence(DiscordIpcClient);

/// Pone la presencia de esa instancia; None si no se ha podido (sin Discord, sin id...). `since`: segundos UNIX del
/// inicio de la partida (para que el contador no vuelva a cero al reponerla).
pub fn start(instance: &str, since: i64) -> Option<Presence> {
    if APP_ID.is_empty() {
        return None;
    }
    let mut client = DiscordIpcClient::new(APP_ID).ok()?;
    client.connect().ok()?;
    let (details, small, small_text) = if instance == "cobblemon" {
        ("Cobblemon", "cobblemon", "Zeres Cobblemon")
    } else {
        ("Zeres Network", "network", "Zeres Network")
    };
    let act = activity::Activity::new()
        .details(details)
        .state("mc.zeres.fun")
        .timestamps(activity::Timestamps::new().start(since))
        .assets(activity::Assets::new().large_image("logo").large_text("Zeres Launcher").small_image(small).small_text(small_text));
    client.set_activity(act).ok()?;
    Some(Presence(client))
}

/// Segundos UNIX de ahora.
pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Presence {
    pub fn stop(mut self) {
        let _ = self.0.clear_activity();
        let _ = self.0.close();
    }
}
