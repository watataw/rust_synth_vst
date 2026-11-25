#[derive(Clone, Copy, Debug)]
pub(super) struct FullscreenTarget {
    pub(super) avail_phys_w: u32,
    pub(super) avail_phys_h: u32,
    pub(super) prev_root_left: i32,
    pub(super) prev_root_top: i32,
}

#[cfg(target_os = "windows")]
mod imp {
    use super::FullscreenTarget;

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

    type SubclassProc = unsafe extern "system" fn(
        hwnd: isize,
        u_msg: u32,
        w_param: usize,
        l_param: isize,
        u_id_subclass: usize,
        dw_ref_data: usize,
    ) -> isize;

    const MONITOR_DEFAULTTONEAREST: u32 = 0x00000002;
    const GW_CHILD: u32 = 5;
    const GA_ROOT: u32 = 2;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const WM_SIZE: u32 = 0x0005;
    const WM_NCDESTROY: u32 = 0x0082;
    const WM_APP_SYNC_CHILD: u32 = 0x8000 + 0x41;
    const SUBCLASS_ID: usize = 0x524D_524B;

    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromWindow(hwnd: isize, dw_flags: u32) -> isize;
        fn GetMonitorInfoW(hmonitor: isize, lpmi: *mut MonitorInfo) -> i32;
        fn GetWindow(hwnd: isize, u_cmd: u32) -> isize;
        fn GetAncestor(hwnd: isize, ga_flags: u32) -> isize;
        fn GetClientRect(hwnd: isize, lp_rect: *mut Rect) -> i32;
        fn GetWindowRect(hwnd: isize, lp_rect: *mut Rect) -> i32;
        fn PostMessageW(hwnd: isize, msg: u32, w_param: usize, l_param: isize) -> i32;
        fn SetWindowPos(
            hwnd: isize,
            hwnd_insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            u_flags: u32,
        ) -> i32;
    }

    #[link(name = "comctl32")]
    extern "system" {
        fn SetWindowSubclass(
            hwnd: isize,
            pfn_subclass: SubclassProc,
            u_id_subclass: usize,
            dw_ref_data: usize,
        ) -> i32;
        fn RemoveWindowSubclass(
            hwnd: isize,
            pfn_subclass: SubclassProc,
            u_id_subclass: usize,
        ) -> i32;
        fn DefSubclassProc(
            hwnd: isize,
            u_msg: u32,
            w_param: usize,
            l_param: isize,
        ) -> isize;
    }

    unsafe fn sync_child_to_parent(parent_hwnd: isize) {
        if parent_hwnd == 0 {
            return;
        }
        let mut parent_rect = Rect::default();
        if GetClientRect(parent_hwnd, &mut parent_rect) == 0 {
            return;
        }
        let target_w = parent_rect.right - parent_rect.left;
        let target_h = parent_rect.bottom - parent_rect.top;
        if target_w <= 0 || target_h <= 0 {
            return;
        }

        let child_hwnd = GetWindow(parent_hwnd, GW_CHILD);
        if child_hwnd == 0 {
            return;
        }
        let mut child_rect = Rect::default();
        if GetClientRect(child_hwnd, &mut child_rect) != 0 {
            let cur_w = child_rect.right - child_rect.left;
            let cur_h = child_rect.bottom - child_rect.top;
            if cur_w != target_w || cur_h != target_h {
                let _ = SetWindowPos(
                    child_hwnd,
                    0,
                    0,
                    0,
                    target_w,
                    target_h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }

    unsafe extern "system" fn parent_subclass_proc(
        hwnd: isize,
        u_msg: u32,
        w_param: usize,
        l_param: isize,
        u_id_subclass: usize,
        _dw_ref_data: usize,
    ) -> isize {
        match u_msg {
            WM_SIZE => {
                let _ = PostMessageW(hwnd, WM_APP_SYNC_CHILD, 0, 0);
            }
            WM_APP_SYNC_CHILD => {
                sync_child_to_parent(hwnd);
                return 0;
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, parent_subclass_proc, u_id_subclass);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, u_msg, w_param, l_param)
    }

    pub(crate) fn install_parent_resize_hook(parent_hwnd: isize) {
        if parent_hwnd == 0 {
            return;
        }
        unsafe {
            let _ = RemoveWindowSubclass(parent_hwnd, parent_subclass_proc, SUBCLASS_ID);
            let _ = SetWindowSubclass(parent_hwnd, parent_subclass_proc, SUBCLASS_ID, 0);
            let _ = PostMessageW(parent_hwnd, WM_APP_SYNC_CHILD, 0, 0);
        }
    }

    pub(crate) fn remove_parent_resize_hook(parent_hwnd: isize) {
        if parent_hwnd == 0 {
            return;
        }
        unsafe {
            let _ = RemoveWindowSubclass(parent_hwnd, parent_subclass_proc, SUBCLASS_ID);
        }
    }

    pub(in crate::ui) fn parent_client_size(parent_hwnd: isize) -> Option<(u32, u32)> {
        if parent_hwnd == 0 {
            return None;
        }
        unsafe {
            let mut rect = Rect::default();
            if GetClientRect(parent_hwnd, &mut rect) != 0 {
                let w = (rect.right - rect.left).max(0) as u32;
                let h = (rect.bottom - rect.top).max(0) as u32;
                if w > 0 && h > 0 {
                    return Some((w, h));
                }
            }
        }
        None
    }

    pub(in crate::ui) fn enter_fullscreen_on_monitor(
        parent_hwnd: isize,
    ) -> Option<FullscreenTarget> {
        if parent_hwnd == 0 {
            return None;
        }
        unsafe {
            let mon = MonitorFromWindow(parent_hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MonitorInfo {
                cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                rc_monitor: Rect::default(),
                rc_work: Rect::default(),
                dw_flags: 0,
            };
            if mon == 0 || GetMonitorInfoW(mon, &mut mi) == 0 {
                return None;
            }

            let work_w = (mi.rc_work.right - mi.rc_work.left).max(560);
            let work_h = (mi.rc_work.bottom - mi.rc_work.top).max(380);

            let root_hwnd = GetAncestor(parent_hwnd, GA_ROOT);
            let mut root_rect = Rect::default();
            let mut parent_client = Rect::default();

            let (frame_w, frame_h, prev_left, prev_top) = if root_hwnd != 0
                && GetWindowRect(root_hwnd, &mut root_rect) != 0
                && GetClientRect(parent_hwnd, &mut parent_client) != 0
            {
                let outer_w = root_rect.right - root_rect.left;
                let outer_h = root_rect.bottom - root_rect.top;
                let client_w = parent_client.right - parent_client.left;
                let client_h = parent_client.bottom - parent_client.top;
                let fw = (outer_w - client_w).clamp(0, 64);
                let fh = (outer_h - client_h).clamp(0, 96);

                let _ = SetWindowPos(
                    root_hwnd,
                    0,
                    mi.rc_work.left,
                    mi.rc_work.top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );

                (fw, fh, root_rect.left, root_rect.top)
            } else {
                (16, 40, mi.rc_work.left, mi.rc_work.top)
            };

            Some(FullscreenTarget {
                avail_phys_w: (work_w - frame_w).max(560) as u32,
                avail_phys_h: (work_h - frame_h).max(380) as u32,
                prev_root_left: prev_left,
                prev_root_top: prev_top,
            })
        }
    }

    pub(in crate::ui) fn restore_root_window_pos(parent_hwnd: isize, left: i32, top: i32) {
        if parent_hwnd == 0 {
            return;
        }
        unsafe {
            let root_hwnd = GetAncestor(parent_hwnd, GA_ROOT);
            if root_hwnd != 0 {
                let _ = SetWindowPos(
                    root_hwnd,
                    0,
                    left,
                    top,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::FullscreenTarget;

    pub(crate) fn install_parent_resize_hook(_parent_hwnd: isize) {}
    pub(crate) fn remove_parent_resize_hook(_parent_hwnd: isize) {}
    pub(in crate::ui) fn parent_client_size(_parent_hwnd: isize) -> Option<(u32, u32)> {
        None
    }
    pub(in crate::ui) fn enter_fullscreen_on_monitor(
        _parent_hwnd: isize,
    ) -> Option<FullscreenTarget> {
        Some(FullscreenTarget {
            avail_phys_w: 1600,
            avail_phys_h: 900,
            prev_root_left: 100,
            prev_root_top: 100,
        })
    }
    pub(in crate::ui) fn restore_root_window_pos(_parent_hwnd: isize, _left: i32, _top: i32) {}
}

pub(crate) fn install_parent_resize_hook(parent_hwnd: isize) {
    imp::install_parent_resize_hook(parent_hwnd);
}

pub(crate) fn remove_parent_resize_hook(parent_hwnd: isize) {
    imp::remove_parent_resize_hook(parent_hwnd);
}

pub(super) fn parent_client_size(parent_hwnd: isize) -> Option<(u32, u32)> {
    imp::parent_client_size(parent_hwnd)
}

pub(super) fn enter_fullscreen_on_monitor(parent_hwnd: isize) -> Option<FullscreenTarget> {
    imp::enter_fullscreen_on_monitor(parent_hwnd)
}

pub(super) fn restore_root_window_pos(parent_hwnd: isize, left: i32, top: i32) {
    imp::restore_root_window_pos(parent_hwnd, left, top);
}