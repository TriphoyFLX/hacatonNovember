//! Windows MIDI mapper. Notes go to the system synthesizer, not to a file or the network.

use windows::Win32::Media::Audio::{
    midiOutClose, midiOutOpen, midiOutReset, midiOutShortMsg, CALLBACK_NULL, HMIDIOUT,
};

pub struct MidiOut {
    raw: usize,
}

unsafe impl Send for MidiOut {}

impl MidiOut {
    pub fn open() -> Option<Self> {
        let mut handle = HMIDIOUT::default();
        let status = unsafe { midiOutOpen(&mut handle, u32::MAX, None, None, CALLBACK_NULL) };
        if status != 0 || handle.is_invalid() {
            return None;
        }
        Some(Self {
            raw: handle.0 as usize,
        })
    }

    fn handle(&self) -> HMIDIOUT {
        HMIDIOUT(self.raw as *mut core::ffi::c_void)
    }

    pub fn short(&self, status: u8, data1: u8, data2: u8) {
        if self.raw == 0 {
            return;
        }
        let message = u32::from(status) | (u32::from(data1) << 8) | (u32::from(data2) << 16);
        unsafe {
            midiOutShortMsg(self.handle(), message);
        }
    }

    pub fn note_on(&self, channel: u8, note: u8, velocity: u8) {
        self.short(0x90 | (channel & 0x0f), note, velocity.min(127));
    }

    pub fn note_off(&self, channel: u8, note: u8) {
        self.short(0x80 | (channel & 0x0f), note, 0);
    }

    pub fn program(&self, channel: u8, program: u8) {
        self.short(0xc0 | (channel & 0x0f), program, 0);
    }

    pub fn control(&self, channel: u8, controller: u8, value: u8) {
        self.short(0xb0 | (channel & 0x0f), controller, value.min(127));
    }

    pub fn silence(&self) {
        if self.raw == 0 {
            return;
        }
        unsafe {
            midiOutReset(self.handle());
        }
    }
}

impl Drop for MidiOut {
    fn drop(&mut self) {
        if self.raw == 0 {
            return;
        }
        let handle = self.handle();
        unsafe {
            midiOutReset(handle);
            midiOutClose(handle);
        }
        self.raw = 0;
    }
}
