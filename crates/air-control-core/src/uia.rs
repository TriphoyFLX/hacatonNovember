//! Screen UI targets for magnetic aim. Windows uses the foreground window tree;
//! other OSes return an empty list in this wave.

#[derive(Debug, Clone)]
pub struct UiTarget {
    pub name: String,
    pub cx: i32,
    pub cy: i32,
    pub w: i32,
    pub h: i32,
}

impl UiTarget {
    pub fn contains_pad(&self, x: i32, y: i32, pad: i32) -> bool {
        let half_w = self.w / 2 + pad;
        let half_h = self.h / 2 + pad;
        (x - self.cx).abs() <= half_w && (y - self.cy).abs() <= half_h
    }

    pub fn distance2(&self, x: i32, y: i32) -> i64 {
        let dx = (x - self.cx) as i64;
        let dy = (y - self.cy) as i64;
        dx * dx + dy * dy
    }
}

pub fn snap_point(x: i32, y: i32, radius_px: f32, targets: &[UiTarget]) -> Option<(i32, i32, String)> {
    let radius = radius_px.max(8.0) as i64;
    let limit = radius * radius;
    let mut best: Option<(i64, &UiTarget)> = None;
    for target in targets {
        if target.w < 8 || target.h < 8 || target.w > 900 || target.h > 500 {
            continue;
        }
        let dist = target.distance2(x, y);
        if dist > limit {
            continue;
        }
        if best.as_ref().is_none_or(|(d, _)| dist < *d) {
            best = Some((dist, target));
        }
    }
    best.map(|(_, target)| (target.cx, target.cy, target.name.clone()))
}

#[cfg(windows)]
pub fn poll_targets() -> Vec<UiTarget> {
    windows_impl::poll_targets()
}

#[cfg(not(windows))]
pub fn poll_targets() -> Vec<UiTarget> {
    Vec::new()
}

#[cfg(windows)]
mod windows_impl {
    use super::UiTarget;
    use std::mem::size_of;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextW,
        IsWindowVisible,
    };

    pub fn poll_targets() -> Vec<UiTarget> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        push_window(hwnd, &mut out, true);
        let _ = unsafe {
            EnumChildWindows(
                Some(hwnd),
                Some(enum_child),
                LPARAM(&mut out as *mut Vec<UiTarget> as isize),
            )
        };
        out.truncate(80);
        out
    }

    unsafe extern "system" fn enum_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<UiTarget>);
        if list.len() >= 80 {
            return BOOL(0);
        }
        push_window(hwnd, list, false);
        BOOL(1)
    }

    fn push_window(hwnd: HWND, out: &mut Vec<UiTarget>, root: bool) {
        unsafe {
            if !root && !IsWindowVisible(hwnd).as_bool() {
                return;
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return;
            }
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            if w < 12 || h < 12 {
                return;
            }
            if !root && (w > 1200 || h > 800) {
                return;
            }
            let mut title = [0u16; 96];
            let title_len = GetWindowTextW(hwnd, &mut title);
            let mut class_buf = [0u16; 64];
            let class_len = GetClassNameW(hwnd, &mut class_buf);
            let title = String::from_utf16_lossy(&title[..title_len as usize]);
            let class = String::from_utf16_lossy(&class_buf[..class_len as usize]);
            let name = if !title.trim().is_empty() {
                title.trim().chars().take(48).collect()
            } else if interesting_class(&class) {
                class.chars().take(32).collect()
            } else if root {
                return;
            } else {
                return;
            };
            if !root && !interesting_class(&class) && title.is_empty() {
                return;
            }
            out.push(UiTarget {
                name,
                cx: rect.left + w / 2,
                cy: rect.top + h / 2,
                w,
                h,
            });
            let _ = size_of::<RECT>();
        }
    }

    fn interesting_class(class: &str) -> bool {
        let c = class.to_ascii_lowercase();
        c.contains("button")
            || c.contains("list")
            || c.contains("tree")
            || c.contains("syslistview")
            || c.contains("directui")
            || c.contains("toolbar")
            || c.contains("tab")
            || c.contains("edit")
            || c.contains("static")
            || c.contains("netui")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_picks_nearest_within_radius() {
        let targets = vec![
            UiTarget {
                name: "A".into(),
                cx: 100,
                cy: 100,
                w: 40,
                h: 20,
            },
            UiTarget {
                name: "B".into(),
                cx: 130,
                cy: 100,
                w: 40,
                h: 20,
            },
        ];
        let hit = snap_point(118, 100, 40.0, &targets).unwrap();
        assert_eq!(hit.2, "B");
        assert!(snap_point(400, 400, 20.0, &targets).is_none());
    }
}
