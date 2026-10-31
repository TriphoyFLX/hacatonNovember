//! Local MIDI out. Windows uses the system mapper; other OSes use midir.

#[cfg(windows)]
mod platform {
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
}

#[cfg(not(windows))]
mod platform {
    use std::sync::Mutex;

    use midir::{MidiOutput, MidiOutputConnection};

    pub struct MidiOut {
        conn: Mutex<MidiOutputConnection>,
    }

    unsafe impl Send for MidiOut {}

    impl MidiOut {
        pub fn open() -> Option<Self> {
            let output = MidiOutput::new("AIR CONTROL").ok()?;
            let ports = output.ports();
            if ports.is_empty() {
                return None;
            }
            let preferred = ports.iter().find(|port| {
                output
                    .port_name(port)
                    .map(|name| {
                        let lower = name.to_ascii_lowercase();
                        lower.contains("synth")
                            || lower.contains("iac")
                            || lower.contains("dls")
                            || lower.contains("gs")
                    })
                    .unwrap_or(false)
            });
            let port = preferred.unwrap_or(&ports[0]);
            let conn = output.connect(port, "AIR CONTROL out").ok()?;
            Some(Self {
                conn: Mutex::new(conn),
            })
        }

        pub fn short(&self, status: u8, data1: u8, data2: u8) {
            if let Ok(mut conn) = self.conn.lock() {
                let _ = conn.send(&[status, data1, data2]);
            }
        }

        pub fn silence(&self) {
            if let Ok(mut conn) = self.conn.lock() {
                let _ = conn.send(&[0xb0, 123, 0]);
                let _ = conn.send(&[0xb9, 123, 0]);
                for note in 0u8..=127 {
                    let _ = conn.send(&[0x80, note, 0]);
                    let _ = conn.send(&[0x89, note, 0]);
                }
            }
        }
    }
}

pub struct MidiOut {
    inner: platform::MidiOut,
}

impl MidiOut {
    pub fn open() -> Option<Self> {
        platform::MidiOut::open().map(|inner| Self { inner })
    }

    pub fn short(&self, status: u8, data1: u8, data2: u8) {
        self.inner.short(status, data1, data2);
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

    pub fn pitch_bend(&self, channel: u8, value: u16) {
        let v = value.min(16_383);
        self.short(
            0xe0 | (channel & 0x0f),
            (v & 0x7f) as u8,
            ((v >> 7) & 0x7f) as u8,
        );
    }

    pub fn silence(&self) {
        self.inner.silence();
    }
}
