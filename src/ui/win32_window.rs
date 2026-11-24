#[cfg(target_os = "windows")]
mod imp {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        cb_size: u32,
        rc_monitor: Rect,
        rc_work: Rect,
        dw_flags: u32,
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 0x00000002;

    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(lp_point: *mut Point) -> i32;
        fn MonitorFromPoint(pt: Point, dw_flags: u32) -> isize;
        fn GetMonitorInfoW(hmonitor: isize, lpmi: *mut MonitorInfo) -> i32;
    }

    pub(in crate::ui) fn current_monitor_work_size() -> (u32, u32) {
        unsafe {
            let mut pt = Point::default();
            let _ = GetCursorPos(&mut pt);
            let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut mi = MonitorInfo {
                cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                rc_monitor: Rect::default(),
                rc_work: Rect::default(),
                dw_flags: 0,
            };
            if mon != 0 && GetMonitorInfoW(mon, &mut mi) != 0 {
                let w = (mi.rc_work.right - mi.rc_work.left).max(560) as u32;
                let h = (mi.rc_work.bottom - mi.rc_work.top).max(380) as u32;
                (w, h)
            } else {
                (1600, 900)
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub(in crate::ui) fn current_monitor_work_size() -> (u32, u32) {
        (1600, 900)
    }
}

pub(super) fn current_monitor_work_size() -> (u32, u32) {
    imp::current_monitor_work_size()
}