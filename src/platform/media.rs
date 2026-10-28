//! Leitura e controle de players que implementam MPRIS 2.
//!
//! Aplicativos podem usar o mesmo modelo para compor controles de mídia sem depender
//! de integrações específicas com Spotify, navegadores ou players locais.

use std::collections::HashMap;
use std::{path::PathBuf, process::Command};

use tiny_skia::{Pixmap, PremultipliedColorU8};
use zbus::{blocking::{Connection, Proxy, fdo::DBusProxy}, zvariant::{OwnedObjectPath, OwnedValue}};

const ROOT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaSession {
    pub bus_name: String,
    pub identity: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub art_url: Option<String>,
    pub track_id: Option<String>,
    pub playback_status: String,
    pub position_us: i64,
    pub duration_us: i64,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_control: bool,
    pub shuffle: bool,
    pub loop_status: String,
    pub tracks: Vec<MediaTrack>,
    pub playlists: Vec<MediaPlaylist>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaTrack {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaPlaylist {
    pub id: String,
    pub name: String,
    pub icon_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaAction {
    Previous,
    PlayPause,
    Stop,
    Next,
    ToggleShuffle,
    CycleRepeat,
    PlayTrack(String),
    ActivatePlaylist(String),
}

fn value_string(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    String::try_from(map.get(key)?.try_clone().ok()?).ok()
}

fn value_strings(map: &HashMap<String, OwnedValue>, key: &str) -> Vec<String> {
    map.get(key).and_then(|v| v.try_clone().ok()).and_then(|v| Vec::<String>::try_from(v).ok()).unwrap_or_default()
}

fn value_object_path(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    OwnedObjectPath::try_from(map.get(key)?.try_clone().ok()?).ok().map(|p| p.to_string())
}

fn track(map: &HashMap<String, OwnedValue>) -> MediaTrack {
    MediaTrack {
        id: value_object_path(map, "mpris:trackid").unwrap_or_default(),
        title: value_string(map, "xesam:title").unwrap_or_default(),
        artists: value_strings(map, "xesam:artist"),
        album: value_string(map, "xesam:album").unwrap_or_default(),
    }
}

fn names(conn: &Connection) -> zbus::Result<Vec<String>> {
    Ok(DBusProxy::new(conn)?.list_names()?.into_iter().map(|n| n.to_string()).filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).collect())
}

/// Lê o player em reprodução ou, se nenhum estiver tocando, o primeiro player ativo.
/// TrackList e Playlists são extensões opcionais: players sem elas continuam válidos.
pub fn active() -> Option<MediaSession> {
    active_with_details(true)
}

/// Lê o player; as listas completas só são pedidas com o painel aberto.
pub fn active_with_details(include_lists: bool) -> Option<MediaSession> {
    let conn = Connection::session().ok()?;
    let players = names(&conn).ok()?;
    let status = |name: &str| Proxy::new(&conn, name.to_string(), ROOT, PLAYER).and_then(|p| p.get_property::<String>("PlaybackStatus"));
    let bus_name = players.iter().find(|n| status(n).is_ok_and(|s| s == "Playing"))
        .or_else(|| players.iter().find(|n| status(n).is_ok_and(|s| s == "Paused")))
        .or(players.first())?.clone();
    read(&conn, bus_name, include_lists).ok()
}

fn read(conn: &Connection, bus_name: String, include_lists: bool) -> zbus::Result<MediaSession> {
    let root = Proxy::new(conn, bus_name.clone(), ROOT, "org.mpris.MediaPlayer2")?;
    let player = Proxy::new(conn, bus_name.clone(), ROOT, PLAYER)?;
    let metadata: HashMap<String, OwnedValue> = player.get_property("Metadata").unwrap_or_default();
    let identity = root.get_property::<String>("Identity").unwrap_or_else(|_| bus_name.trim_start_matches("org.mpris.MediaPlayer2.").to_string());
    let duration_us = metadata.get("mpris:length").and_then(|v| i64::try_from(v).ok()).unwrap_or_default();
    let playback_status = player.get_property("PlaybackStatus").unwrap_or_else(|_| "Stopped".into());
    let mut session = MediaSession {
        bus_name: bus_name.clone(), identity,
        title: value_string(&metadata, "xesam:title").unwrap_or_default(),
        artists: value_strings(&metadata, "xesam:artist"),
        album: value_string(&metadata, "xesam:album").unwrap_or_default(),
        art_url: value_string(&metadata, "mpris:artUrl"),
        track_id: value_object_path(&metadata, "mpris:trackid"),
        playback_status,
        position_us: player.get_property("Position").unwrap_or_default(),
        duration_us,
        can_play: player.get_property("CanPlay").unwrap_or(false),
        can_pause: player.get_property("CanPause").unwrap_or(false),
        can_go_next: player.get_property("CanGoNext").unwrap_or(false),
        can_go_previous: player.get_property("CanGoPrevious").unwrap_or(false),
        can_control: player.get_property("CanControl").unwrap_or(false),
        shuffle: player.get_property("Shuffle").unwrap_or(false),
        loop_status: player.get_property("LoopStatus").unwrap_or_else(|_| "None".into()),
        ..MediaSession::default()
    };
    if include_lists && root.get_property::<bool>("HasTrackList").unwrap_or(false) {
        if let Ok(list) = Proxy::new(conn, bus_name.clone(), ROOT, "org.mpris.MediaPlayer2.TrackList") {
            let ids: Vec<OwnedObjectPath> = list.get_property("Tracks").unwrap_or_default();
            let ids: Vec<OwnedObjectPath> = ids.into_iter().take(12).collect();
            if !ids.is_empty() {
                if let Ok(items) = list.call::<_, _, Vec<HashMap<String, OwnedValue>>>("GetTracksMetadata", &(ids.clone())) {
                    session.tracks = items.iter().map(track).collect();
                }
            }
        }
    }
    if include_lists && let Ok(playlists) = Proxy::new(conn, bus_name, ROOT, "org.mpris.MediaPlayer2.Playlists") {
        if let Ok(items) = playlists.call::<_, _, Vec<(OwnedObjectPath, String, String)>>("GetPlaylists", &(0u32, 8u32, "Alphabetical", false)) {
            session.playlists = items.into_iter().map(|(id, name, icon_url)| MediaPlaylist {
                id: id.to_string(), name, icon_url,
            }).collect();
        }
    }
    Ok(session)
}

/// Executa uma ação do componente no player identificado pela sessão.
pub fn command(session: &MediaSession, action: MediaAction) -> zbus::Result<()> {
    let conn = Connection::session()?;
    let player = Proxy::new(&conn, session.bus_name.clone(), ROOT, PLAYER)?;
    match action {
        MediaAction::Previous => player.call("Previous", &()),
        MediaAction::PlayPause => player.call("PlayPause", &()),
        MediaAction::Stop => player.call("Stop", &()),
        MediaAction::Next => player.call("Next", &()),
        MediaAction::ToggleShuffle => Ok(player.set_property("Shuffle", !session.shuffle)?),
        MediaAction::CycleRepeat => {
            let next = match session.loop_status.as_str() { "None" => "Playlist", "Playlist" => "Track", _ => "None" };
            Ok(player.set_property("LoopStatus", next)?)
        }
        MediaAction::PlayTrack(id) => {
            let list = Proxy::new(&conn, session.bus_name.clone(), ROOT, "org.mpris.MediaPlayer2.TrackList")?;
            list.call("GoTo", &(OwnedObjectPath::try_from(id).map_err(|e| zbus::Error::Failure(e.to_string()))?))
        }
        MediaAction::ActivatePlaylist(id) => {
            let playlists = Proxy::new(&conn, session.bus_name.clone(), ROOT, "org.mpris.MediaPlayer2.Playlists")?;
            playlists.call("ActivatePlaylist", &(OwnedObjectPath::try_from(id).map_err(|e| zbus::Error::Failure(e.to_string()))?))
        }
    }
}

/// Define a posição na faixa (em microssegundos). O player pode rejeitar seeking.
pub fn seek(session: &MediaSession, position_us: i64) -> zbus::Result<()> {
    let Some(id) = &session.track_id else { return Ok(()) };
    let conn = Connection::session()?;
    let player = Proxy::new(&conn, session.bus_name.clone(), ROOT, PLAYER)?;
    let id = OwnedObjectPath::try_from(id.clone()).map_err(|e| zbus::Error::Failure(e.to_string()))?;
    player.call("SetPosition", &(id, position_us.clamp(0, session.duration_us.max(0))))
}

/// Carrega e reduz a capa do player. Só lê `file://` e URLs HTTP(S); respostas remotas
/// têm limite de tamanho e tempo para não travar a interface nem consumir memória sem limite.
pub fn load_artwork(uri: &str, size: u32) -> Option<Pixmap> {
    let bytes = if let Some(path) = uri.strip_prefix("file://") {
        std::fs::read(decode_file_uri(path)?).ok()?
    } else if uri.starts_with("https://") || uri.starts_with("http://") {
        let output = Command::new("curl").args([
            "--silent", "--show-error", "--fail", "--location", "--max-time", "4",
            "--max-filesize", "4194304", "--proto", "=http,https", "--proto-redir", "=http,https", "--", uri,
        ]).output().ok()?;
        if !output.status.success() || output.stdout.len() > 4 * 1024 * 1024 { return None; }
        output.stdout
    } else {
        return None;
    };
    let decoded = image::load_from_memory(&bytes).ok()?.thumbnail(size, size).to_rgba8();
    let mut pixmap = Pixmap::new(size, size)?;
    let (ox, oy) = ((size - decoded.width()) / 2, (size - decoded.height()) / 2);
    for (y, row) in decoded.rows().enumerate() {
        for (x, pixel) in row.enumerate() {
            let [r, g, b, a] = pixel.0;
            pixmap.pixels_mut()[((y as u32 + oy) * size + x as u32 + ox) as usize] = PremultipliedColorU8::from_rgba(
                (r as u16 * a as u16 / 255) as u8, (g as u16 * a as u16 / 255) as u8,
                (b as u16 * a as u16 / 255) as u8, a,
            )?;
        }
    }
    Some(pixmap)
}

fn decode_file_uri(path: &str) -> Option<PathBuf> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_file_uri_percent_decodes_spaces() {
        assert_eq!(decode_file_uri("/tmp/album%20cover.png").unwrap(), PathBuf::from("/tmp/album cover.png"));
        assert!(decode_file_uri("/tmp/%GG.png").is_none());
    }

}
