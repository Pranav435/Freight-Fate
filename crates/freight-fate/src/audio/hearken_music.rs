//! Music on the Hearken backend: shipped tracks and the player's own files,
//! faded the way [`BassBackend`](super::BassBackend) fades them.
//!
//! Every track is played once, never looped: the station rotation and the
//! playlist decide what comes next. A new track fades the old one out over
//! 800 ms while it fades itself in to the music level.

use std::path::Path;

use hearken::audio::{Bus, Sound, Start};

use super::hearken_backend::HearkenBackend;
use super::hearken_clips::FfClips;
use super::AudioError;

/// How long the outgoing track takes to fade when another replaces it.
const MUSIC_CROSSFADE_OUT_MS: u32 = 800;

impl HearkenBackend {
    pub(super) fn play_music(&mut self, track: &str, fade_ms: u32) {
        if self.music_track.as_deref() == Some(track) {
            return;
        }
        self.play_music_at(track, fade_ms, 0.0);
    }

    /// Play a shipped track from `start_s` seconds in. Like BASS, this does
    /// not short-circuit on the track already being the one loaded: the
    /// point of the call is the position.
    pub(super) fn play_music_at(&mut self, track: &str, fade_ms: u32, start_s: f64) {
        let Some((data, _ext)) = FfClips::music(track) else {
            log::warn!("Missing music track: {track}");
            return;
        };
        self.fade_out_music();
        let start = Start::ONCE.volume(0.0).from_s(start_s.max(0.0));
        match self.audio.try_play_clip(track, data, Bus::MUSIC, start) {
            Ok(sound) => {
                self.fade_in_music(sound, fade_ms);
                self.music_track = Some(track.to_string());
            }
            Err(err) => log::warn!("Could not open stream: {track} ({err})"),
        }
    }

    /// Play one of the player's own files. Fails when the file cannot be
    /// read or decoded, so the radio layer can skip to the next entry.
    pub(super) fn play_music_file(&mut self, path: &str, fade_ms: u32) -> Result<(), AudioError> {
        let key = format!("file:{path}");
        let data: std::sync::Arc<[u8]> = match std::fs::read(Path::new(path)) {
            Ok(data) => data.into(),
            Err(_) => return Err(AudioError::new(format!("could not read {path}"))),
        };
        self.fade_out_music();
        let start = Start::ONCE.volume(0.0);
        match self.audio.try_play_clip(&key, data, Bus::MUSIC, start) {
            Ok(sound) => {
                self.fade_in_music(sound, fade_ms);
                self.music_track = Some(key);
                Ok(())
            }
            Err(_) => Err(AudioError::new(format!("could not decode {path}"))),
        }
    }

    pub(super) fn music_playing(&self) -> bool {
        self.music_sound
            .is_some_and(|sound| self.audio.is_playing(sound))
    }

    /// The current track's length, as read when it started: never the
    /// outgoing one's.
    pub(super) fn music_length_s(&self) -> Option<f64> {
        self.music_sound?;
        self.music_length.filter(|s| *s > 0.0)
    }

    pub(super) fn stop_music(&mut self, fade_ms: u32) {
        let Some(sound) = self.music_sound.take() else {
            return;
        };
        self.audio.stop(sound, fade_ms);
        self.music_track = None;
        self.music_length = None;
    }

    fn fade_out_music(&mut self) {
        if let Some(sound) = self.music_sound.take() {
            self.audio.stop(sound, MUSIC_CROSSFADE_OUT_MS);
            self.music_track = None;
            self.music_length = None;
        }
    }

    fn fade_in_music(&mut self, sound: Sound, fade_ms: u32) {
        let level = self.buses.music_level() as f32;
        self.audio.set_volume_over(sound, level, fade_ms);
        self.music_length = self.audio.length_s(sound);
        self.music_sound = Some(sound);
    }
}
