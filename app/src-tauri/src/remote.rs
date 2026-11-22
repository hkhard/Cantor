//! PLAY-12: lock screen, Control Centre and media keys (AirPods, Bluetooth
//! remotes) control playback; the lock screen shows the score title and the
//! position. Background audio itself comes from `UIBackgroundModes: audio`
//! and the Playback audio session (set in main.mm).

use cantor_audio::{Engine, SoundFontSynth};
use std::sync::{Arc, Mutex};

// The MediaPlayer constants (MPMediaItemPropertyTitle, ...) are data
// symbols, so the framework must be linked explicitly.
#[cfg(target_os = "ios")]
#[link(name = "MediaPlayer", kind = "framework")]
extern "C" {}

pub type SharedEngine = Arc<Mutex<Option<Arc<Mutex<Engine<SoundFontSynth>>>>>>;

#[cfg(target_os = "ios")]
pub fn install(engine: SharedEngine) {
    use block2::RcBlock;
    use objc2_media_player::{MPRemoteCommandCenter, MPRemoteCommandHandlerStatus};

    let with = move |f: fn(&mut Engine<SoundFontSynth>)| {
        let engine = engine.clone();
        RcBlock::new(move |_ev| {
            let slot = engine.lock().unwrap();
            match slot.as_ref() {
                Some(e) => {
                    f(&mut e.lock().unwrap());
                    MPRemoteCommandHandlerStatus::Success
                }
                None => MPRemoteCommandHandlerStatus::NoActionableNowPlayingItem,
            }
        })
    };
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        let play = with(|e| {
            if e.finished() {
                e.seek(0.0);
            }
            e.play()
        });
        let pause = with(|e| e.pause());
        let toggle = with(|e| {
            if e.is_playing() {
                e.pause()
            } else {
                if e.finished() {
                    e.seek(0.0);
                }
                e.play()
            }
        });
        let _ = center.playCommand().addTargetWithHandler(&play);
        let _ = center.pauseCommand().addTargetWithHandler(&pause);
        let _ = center
            .togglePlayPauseCommand()
            .addTargetWithHandler(&toggle);
        // Handlers must outlive the app: leak the blocks deliberately.
        std::mem::forget((play, pause, toggle));
    }
}

#[cfg(not(target_os = "ios"))]
pub fn install(_engine: SharedEngine) {}

/// Update the lock-screen card: title, length, position, playing or not.
#[cfg(target_os = "ios")]
pub fn now_playing(title: &str, duration: f64, position: f64, playing: bool) {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
        MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingInfoPropertyPlaybackRate,
    };
    unsafe {
        let title: Retained<AnyObject> =
            Retained::into_super(Retained::into_super(NSString::from_str(title)));
        let dur: Retained<AnyObject> = Retained::into_super(Retained::into_super(
            Retained::into_super(NSNumber::new_f64(duration)),
        ));
        let pos: Retained<AnyObject> = Retained::into_super(Retained::into_super(
            Retained::into_super(NSNumber::new_f64(position)),
        ));
        let rate: Retained<AnyObject> = Retained::into_super(Retained::into_super(
            Retained::into_super(NSNumber::new_f64(if playing { 1.0 } else { 0.0 })),
        ));
        let keys: [&NSString; 4] = [
            MPMediaItemPropertyTitle,
            MPMediaItemPropertyPlaybackDuration,
            MPNowPlayingInfoPropertyElapsedPlaybackTime,
            MPNowPlayingInfoPropertyPlaybackRate,
        ];
        let info = NSDictionary::from_retained_objects(&keys, &[title, dur, pos, rate]);
        MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(Some(&info));
    }
}

#[cfg(not(target_os = "ios"))]
pub fn now_playing(_title: &str, _duration: f64, _position: f64, _playing: bool) {}
