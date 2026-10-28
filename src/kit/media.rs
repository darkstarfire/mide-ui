//! Componentes reutilizáveis para controlar e acompanhar mídia.

use tiny_skia::Pixmap;

use crate::platform::media::{MediaAction, MediaSession};

use super::{tr, ButtonStyle, Ui};

impl Ui<'_> {
    /// Cartão acessível de reprodução. Devolve as ações escolhidas; o app as executa
    /// com `platform::media::command` no player correspondente.
    pub fn media_controls(&mut self, session: &MediaSession, artwork: Option<&Pixmap>) -> Vec<MediaAction> {
        let mut actions = Vec::new();
        self.card(|ui| {
            ui.heading(&tr("Now Playing"));
            ui.image_centered(artwork, "audio-x-generic", 96);
            ui.strong(if session.title.is_empty() { &session.identity } else { &session.title });
            if !session.artists.is_empty() {
                ui.label(&session.artists.join(", "));
            }
            if !session.album.is_empty() {
                ui.note(&session.album);
            }
            if session.duration_us > 0 {
                ui.progress_bar(Some((session.position_us as f32 / session.duration_us as f32).clamp(0.0, 1.0)));
            }
            ui.row(|ui| {
                // Botões só de ícone, com dica (I2).
                let r = ui.icon_button("media-skip-backward-symbolic", "|◀", session.can_go_previous);
                ui.tooltip(&r, "Previous");
                if r.clicked() { actions.push(MediaAction::Previous); }
                let playing = session.playback_status == "Playing";
                let r = ui.icon_button(if playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, if playing { "Ⅱ" } else { "▶" }, session.can_play || session.can_pause);
                ui.tooltip(&r, if playing { "Pause" } else { "Play" });
                if r.clicked() { actions.push(MediaAction::PlayPause); }
                let r = ui.icon_button("media-playback-stop-symbolic", "■", session.can_control);
                ui.tooltip(&r, "Stop");
                if r.clicked() { actions.push(MediaAction::Stop); }
                let r = ui.icon_button("media-skip-forward-symbolic", "▶|", session.can_go_next);
                ui.tooltip(&r, "Next");
                if r.clicked() { actions.push(MediaAction::Next); }
            });
            ui.row(|ui| {
                let r = ui.icon_toggle("media-playlist-shuffle-symbolic", "⇄", session.shuffle);
                ui.tooltip(&r, "Shuffle");
                if r.clicked() { actions.push(MediaAction::ToggleShuffle); }
                let repeat = match session.loop_status.as_str() { "Playlist" => tr("Repeat Playlist"), "Track" => tr("Repeat Track"), _ => tr("Repeat") };
                if ui.button_styled(&repeat, if session.loop_status == "None" { ButtonStyle::Bordered } else { ButtonStyle::Prominent }).clicked() { actions.push(MediaAction::CycleRepeat); }
            });
            if !session.playlists.is_empty() {
                ui.heading(&tr("Playlists"));
                ui.scroll_area(132.0, |ui| {
                    for playlist in &session.playlists {
                        if ui.list_item(&playlist.name, "").clicked() { actions.push(MediaAction::ActivatePlaylist(playlist.id.clone())); }
                    }
                });
            }
            if !session.tracks.is_empty() {
                ui.heading(&tr("Up Next"));
                ui.scroll_area(156.0, |ui| {
                    for track in &session.tracks {
                        let detail = track.artists.join(", ");
                        let row = ui.list_item(&track.title, &detail);
                        if row.clicked() { actions.push(MediaAction::PlayTrack(track.id.clone())); }
                    }
                });
            }
        });
        actions
    }
}
