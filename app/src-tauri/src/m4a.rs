//! PLAY-11: AAC in .m4a via AudioToolbox's ExtAudioFile (iOS and macOS).
//! The engine renders 16-bit stereo PCM (as for WAV); this writes it to an
//! AAC file at 256 kbit/s, about a tenth of the WAV size, which suits
//! sharing practice tracks in Messages and Mail.

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod ffi {
    #![allow(non_snake_case, non_upper_case_globals)]
    use std::ffi::c_void;

    pub type OSStatus = i32;
    pub type ExtAudioFileRef = *mut c_void;
    pub type CFURLRef = *const c_void;
    pub type CFStringRef = *const c_void;
    pub type CFAllocatorRef = *const c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct AudioStreamBasicDescription {
        pub mSampleRate: f64,
        pub mFormatID: u32,
        pub mFormatFlags: u32,
        pub mBytesPerPacket: u32,
        pub mFramesPerPacket: u32,
        pub mBytesPerFrame: u32,
        pub mChannelsPerFrame: u32,
        pub mBitsPerChannel: u32,
        pub mReserved: u32,
    }

    #[repr(C)]
    pub struct AudioBuffer {
        pub mNumberChannels: u32,
        pub mDataByteSize: u32,
        pub mData: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioBufferList {
        pub mNumberBuffers: u32,
        pub mBuffers: [AudioBuffer; 1],
    }

    pub const kAudioFormatLinearPCM: u32 = u32::from_be_bytes(*b"lpcm");
    pub const kAudioFormatMPEG4AAC: u32 = u32::from_be_bytes(*b"aac ");
    pub const kAudioFileM4AType: u32 = u32::from_be_bytes(*b"m4af");
    pub const kAudioFormatFlagIsSignedInteger: u32 = 1 << 2;
    pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;
    pub const kExtAudioFileProperty_ClientDataFormat: u32 = u32::from_be_bytes(*b"cfmt");
    pub const kExtAudioFileProperty_AudioConverter: u32 = u32::from_be_bytes(*b"acnv");
    pub const kAudioConverterEncodeBitRate: u32 = u32::from_be_bytes(*b"brat");
    pub const kAudioFileFlags_EraseFile: u32 = 1;
    pub const kCFURLPOSIXPathStyle: isize = 0;
    pub const kCFStringEncodingUTF8: u32 = 0x0800_0100;

    #[link(name = "AudioToolbox", kind = "framework")]
    extern "C" {
        pub fn ExtAudioFileCreateWithURL(
            url: CFURLRef,
            file_type: u32,
            format: *const AudioStreamBasicDescription,
            layout: *const c_void,
            flags: u32,
            out: *mut ExtAudioFileRef,
        ) -> OSStatus;
        pub fn ExtAudioFileSetProperty(
            f: ExtAudioFileRef,
            id: u32,
            size: u32,
            data: *const c_void,
        ) -> OSStatus;
        pub fn ExtAudioFileGetProperty(
            f: ExtAudioFileRef,
            id: u32,
            size: *mut u32,
            data: *mut c_void,
        ) -> OSStatus;
        pub fn ExtAudioFileWrite(
            f: ExtAudioFileRef,
            frames: u32,
            data: *const AudioBufferList,
        ) -> OSStatus;
        pub fn ExtAudioFileDispose(f: ExtAudioFileRef) -> OSStatus;
        pub fn AudioConverterSetProperty(
            c: *mut c_void,
            id: u32,
            size: u32,
            data: *const c_void,
        ) -> OSStatus;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFStringCreateWithBytes(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            len: isize,
            encoding: u32,
            external: u8,
        ) -> CFStringRef;
        pub fn CFURLCreateWithFileSystemPath(
            alloc: CFAllocatorRef,
            path: CFStringRef,
            style: isize,
            is_dir: u8,
        ) -> CFURLRef;
        pub fn CFRelease(cf: *const c_void);
    }
}

/// Encode interleaved 16-bit stereo PCM at `sample_rate` as AAC into an
/// .m4a file at `path`.
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub fn write_m4a(path: &std::path::Path, pcm: &[i16], sample_rate: u32) -> Result<(), String> {
    use ffi::*;
    use std::ffi::c_void;
    let p = path.to_string_lossy();
    // SAFETY: plain CoreFoundation / AudioToolbox calls with valid pointers;
    // every created object is released or disposed below.
    unsafe {
        let s = CFStringCreateWithBytes(
            std::ptr::null(),
            p.as_ptr(),
            p.len() as isize,
            kCFStringEncodingUTF8,
            0,
        );
        if s.is_null() {
            return Err("m4a: bad path".into());
        }
        let url = CFURLCreateWithFileSystemPath(std::ptr::null(), s, kCFURLPOSIXPathStyle, 0);
        CFRelease(s);
        if url.is_null() {
            return Err("m4a: bad path".into());
        }
        let aac = AudioStreamBasicDescription {
            mSampleRate: sample_rate as f64,
            mFormatID: kAudioFormatMPEG4AAC,
            mChannelsPerFrame: 2,
            ..Default::default()
        };
        let mut file: ExtAudioFileRef = std::ptr::null_mut();
        let st = ExtAudioFileCreateWithURL(
            url,
            kAudioFileM4AType,
            &aac,
            std::ptr::null(),
            kAudioFileFlags_EraseFile,
            &mut file,
        );
        CFRelease(url);
        if st != 0 {
            return Err(format!("m4a: create failed ({st})"));
        }
        let client = AudioStreamBasicDescription {
            mSampleRate: sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
            mBytesPerPacket: 4,
            mFramesPerPacket: 1,
            mBytesPerFrame: 4,
            mChannelsPerFrame: 2,
            mBitsPerChannel: 16,
            mReserved: 0,
        };
        let mut st = ExtAudioFileSetProperty(
            file,
            kExtAudioFileProperty_ClientDataFormat,
            std::mem::size_of::<AudioStreamBasicDescription>() as u32,
            &client as *const _ as *const c_void,
        );
        if st == 0 {
            // 256 kbit/s; keep the encoder default if the property is refused.
            let mut conv: *mut c_void = std::ptr::null_mut();
            let mut size = std::mem::size_of::<*mut c_void>() as u32;
            if ExtAudioFileGetProperty(
                file,
                kExtAudioFileProperty_AudioConverter,
                &mut size,
                &mut conv as *mut _ as *mut c_void,
            ) == 0
                && !conv.is_null()
            {
                let rate: u32 = 256_000;
                AudioConverterSetProperty(
                    conv,
                    kAudioConverterEncodeBitRate,
                    4,
                    &rate as *const _ as *const c_void,
                );
                // Tell ExtAudioFile the converter changed.
                let null: *const c_void = std::ptr::null();
                ExtAudioFileSetProperty(
                    file,
                    u32::from_be_bytes(*b"acfg"),
                    std::mem::size_of::<*const c_void>() as u32,
                    &null as *const _ as *const c_void,
                );
            }
        }
        let frames_per_chunk = 4096usize;
        for chunk in pcm.chunks(frames_per_chunk * 2) {
            if st != 0 {
                break;
            }
            let list = AudioBufferList {
                mNumberBuffers: 1,
                mBuffers: [AudioBuffer {
                    mNumberChannels: 2,
                    mDataByteSize: (chunk.len() * 2) as u32,
                    mData: chunk.as_ptr() as *mut c_void,
                }],
            };
            st = ExtAudioFileWrite(file, (chunk.len() / 2) as u32, &list);
        }
        let d = ExtAudioFileDispose(file);
        if st != 0 {
            return Err(format!("m4a: encode failed ({st})"));
        }
        if d != 0 {
            return Err(format!("m4a: finish failed ({d})"));
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
pub fn write_m4a(_path: &std::path::Path, _pcm: &[i16], _sample_rate: u32) -> Result<(), String> {
    Err("M4A export needs iOS or macOS".into())
}

/// Interleaved 16-bit samples and sample rate from a 16-bit PCM WAV made by
/// `cantor_audio::export_wav`.
pub fn wav_pcm(wav: &[u8]) -> Option<(Vec<i16>, u32)> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let mut i = 12;
    let mut rate = 0u32;
    while i + 8 <= wav.len() {
        let id = &wav[i..i + 4];
        let size = u32::from_le_bytes(wav[i + 4..i + 8].try_into().ok()?) as usize;
        let body = wav.get(i + 8..i + 8 + size)?;
        if id == b"fmt " {
            rate = u32::from_le_bytes(body.get(4..8)?.try_into().ok()?);
        } else if id == b"data" {
            let pcm = body
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            return Some((pcm, rate));
        }
        i += 8 + size + (size & 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_11_m4a_from_rendered_wav() {
        // Three seconds of a 440 Hz tone at 44.1 kHz, as export_wav would write.
        let sr = 44_100u32;
        let mut pcm = Vec::new();
        for n in 0..sr * 3 {
            let v =
                ((n as f64 * 440.0 * std::f64::consts::TAU / sr as f64).sin() * 12_000.0) as i16;
            pcm.push(v);
            pcm.push(v);
        }
        let mut wav = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&[1, 0, 2, 0]);
        wav.extend_from_slice(&sr.to_le_bytes());
        wav.extend_from_slice(&(sr * 4).to_le_bytes());
        wav.extend_from_slice(&[4, 0, 16, 0]);
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&((pcm.len() * 2) as u32).to_le_bytes());
        for s in &pcm {
            wav.extend_from_slice(&s.to_le_bytes());
        }
        let (back, rate) = wav_pcm(&wav).unwrap();
        assert_eq!((back.len(), rate), (pcm.len(), sr));
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            let out =
                std::env::temp_dir().join(format!("cantor-play11-{}.m4a", std::process::id()));
            write_m4a(&out, &back, rate).unwrap();
            let bytes = std::fs::read(&out).unwrap();
            // An MPEG-4 file: 'ftyp' box near the start, much smaller than the
            // WAV (256 kbit/s vs 1411 kbit/s).
            assert_eq!(&bytes[4..8], b"ftyp");
            assert!(
                bytes.len() < wav.len() / 3,
                "{} vs {}",
                bytes.len(),
                wav.len()
            );
            // PRD: length equals the score length +- 10 ms (AAC adds encoder
            // priming, which afinfo's estimate excludes). Checked with afinfo.
            #[cfg(target_os = "macos")]
            {
                let o = std::process::Command::new("afinfo")
                    .arg(&out)
                    .output()
                    .unwrap();
                let s = String::from_utf8_lossy(&o.stdout);
                let d = s
                    .lines()
                    .find_map(|l| l.trim().strip_prefix("estimated duration:"))
                    .expect("afinfo reports a duration");
                let d: f64 = d.trim().trim_end_matches(" sec").trim().parse().unwrap();
                assert!((d - 3.0).abs() <= 0.01, "duration {d}");
            }
            let _ = std::fs::remove_file(out);
        }
    }
}
