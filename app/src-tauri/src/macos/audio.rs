//! System output volume and mute through CoreAudio, for the Volume action.
//! The volume is the "virtual main volume", the one the menu bar and volume
//! keys move, read and written with the AudioObject API (Apple's replacement
//! for the deprecated AudioHardwareService calls).

use std::ffi::c_void;
use std::ptr::{self, NonNull};

use objc2_audio_toolbox::kAudioHardwareServiceDeviceProperty_VirtualMainVolume;
use objc2_core_audio::{
    kAudioDevicePropertyMute, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal,
    kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject, AudioObjectGetPropertyData,
    AudioObjectHasProperty, AudioObjectID, AudioObjectPropertyAddress, AudioObjectSetPropertyData,
};

/// Why a volume or mute call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// There's no default output device (no audio hardware, as on CI runners).
    NoOutputDevice,
    /// The output device doesn't expose this control (some HDMI and USB outputs).
    Unsupported(&'static str),
    /// CoreAudio returned an error status.
    Status { call: &'static str, status: i32 },
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOutputDevice => write!(f, "no default audio output device"),
            Self::Unsupported(what) => write!(f, "the output device has no {what} control"),
            Self::Status { call, status } => write!(f, "{call} failed with status {status}"),
        }
    }
}

impl std::error::Error for AudioError {}

fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn check(call: &'static str, status: i32) -> Result<(), AudioError> {
    if status == 0 {
        Ok(())
    } else {
        Err(AudioError::Status { call, status })
    }
}

fn default_output() -> Result<AudioObjectID, AudioError> {
    let addr = address(
        kAudioHardwarePropertyDefaultOutputDevice,
        kAudioObjectPropertyScopeGlobal,
    );
    let mut device: AudioObjectID = 0;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;
    // SAFETY: the address, size, and output pointers are valid for the call and
    // `size` matches the u32 the property holds.
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&addr),
            0,
            ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut device).cast::<c_void>(),
        )
    };
    check("get default output device", status)?;
    if device == 0 {
        return Err(AudioError::NoOutputDevice);
    }
    Ok(device)
}

/// Output volume, 0.0 to 1.0.
pub fn volume() -> Result<f32, AudioError> {
    let device = default_output()?;
    let addr = address(
        kAudioHardwareServiceDeviceProperty_VirtualMainVolume,
        kAudioObjectPropertyScopeOutput,
    );
    // SAFETY: valid device ID and address pointer.
    if !unsafe { AudioObjectHasProperty(device, NonNull::from(&addr)) } {
        return Err(AudioError::Unsupported("volume"));
    }
    let mut value: f32 = 0.0;
    let mut size = std::mem::size_of::<f32>() as u32;
    // SAFETY: the property is a Float32; pointers are valid for the call.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            NonNull::from(&addr),
            0,
            ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    check("get volume", status)?;
    Ok(value)
}

/// Sets the output volume, clamped to 0.0 to 1.0.
pub fn set_volume(value: f32) -> Result<(), AudioError> {
    let device = default_output()?;
    let addr = address(
        kAudioHardwareServiceDeviceProperty_VirtualMainVolume,
        kAudioObjectPropertyScopeOutput,
    );
    // SAFETY: valid device ID and address pointer.
    if !unsafe { AudioObjectHasProperty(device, NonNull::from(&addr)) } {
        return Err(AudioError::Unsupported("volume"));
    }
    let value: f32 = if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    };
    // SAFETY: the property is a Float32 of the given size; pointers are valid.
    let status = unsafe {
        AudioObjectSetPropertyData(
            device,
            NonNull::from(&addr),
            0,
            ptr::null(),
            std::mem::size_of::<f32>() as u32,
            NonNull::from(&value).cast::<c_void>(),
        )
    };
    check("set volume", status)
}

/// Whether the output is muted.
pub fn muted() -> Result<bool, AudioError> {
    let device = default_output()?;
    let addr = address(kAudioDevicePropertyMute, kAudioObjectPropertyScopeOutput);
    // SAFETY: valid device ID and address pointer.
    if !unsafe { AudioObjectHasProperty(device, NonNull::from(&addr)) } {
        return Err(AudioError::Unsupported("mute"));
    }
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: the property is a UInt32; pointers are valid for the call.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            NonNull::from(&addr),
            0,
            ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast::<c_void>(),
        )
    };
    check("get mute", status)?;
    Ok(value != 0)
}

/// Mutes or unmutes the output.
pub fn set_muted(muted: bool) -> Result<(), AudioError> {
    let device = default_output()?;
    let addr = address(kAudioDevicePropertyMute, kAudioObjectPropertyScopeOutput);
    // SAFETY: valid device ID and address pointer.
    if !unsafe { AudioObjectHasProperty(device, NonNull::from(&addr)) } {
        return Err(AudioError::Unsupported("mute"));
    }
    let value: u32 = u32::from(muted);
    // SAFETY: the property is a UInt32 of the given size; pointers are valid.
    let status = unsafe {
        AudioObjectSetPropertyData(
            device,
            NonNull::from(&addr),
            0,
            ptr::null(),
            std::mem::size_of::<u32>() as u32,
            NonNull::from(&value).cast::<c_void>(),
        )
    };
    check("set mute", status)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skips (returns None) where there's no usable output, as on CI runners;
    /// anywhere else a failure is a real failure.
    fn usable<T>(r: Result<T, AudioError>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(AudioError::NoOutputDevice | AudioError::Unsupported(_)) => {
                eprintln!("skipped: {}", r.err().unwrap());
                None
            }
            Err(e) => panic!("{e}"),
        }
    }

    // Exactly one test touches the real volume and mute, in sequence, and
    // restores both: tests run in parallel, and two tests changing the system
    // volume at once race (that happened).
    #[test]
    fn volume_and_mute_round_trip_and_restore() {
        let Some(v0) = usable(volume()) else { return };
        let target = if v0 > 0.5 { v0 - 0.125 } else { v0 + 0.125 };
        set_volume(target).expect("set volume");
        let v1 = volume().expect("read volume back");
        set_volume(v0).expect("restore volume");
        assert!((v1 - target).abs() < 0.03, "set {target}, read back {v1}");
        assert!((volume().expect("read restored") - v0).abs() < 0.03);

        // Out of range values clamp instead of erroring.
        set_volume(7.0).expect("clamped high");
        let high = volume().expect("read clamped");
        set_volume(v0).expect("restore after clamp");
        assert!(high > 0.97, "clamped to 1.0, read {high}");

        let Some(m0) = usable(muted()) else { return };
        set_muted(!m0).expect("toggle mute");
        let m1 = muted().expect("read mute back");
        set_muted(m0).expect("restore mute");
        assert_eq!(m1, !m0);
        assert_eq!(muted().expect("read restored mute"), m0);
    }
}
