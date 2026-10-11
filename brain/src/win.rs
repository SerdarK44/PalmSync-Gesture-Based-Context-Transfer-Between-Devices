//! Small Win32 helpers: open things the way Explorer would (without a
//! shell in between), ask Windows which program handles a file type, and
//! take a screenshot of one window.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    type Hwnd = *mut c_void;
    type Hdc = *mut c_void;
    type Hgdi = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_ppm: i32,
        y_ppm: i32,
        clr_used: u32,
        clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 1],
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn PrintWindow(hwnd: Hwnd, hdc: Hdc, flags: u32) -> i32;
        fn GetDC(hwnd: Hwnd) -> Hdc;
        fn ReleaseDC(hwnd: Hwnd, hdc: Hdc) -> i32;
        fn IsIconic(hwnd: Hwnd) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn SetProcessDpiAwarenessContext(value: isize) -> i32;
        fn SetProcessDPIAware() -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
        fn CreateCompatibleBitmap(hdc: Hdc, w: i32, h: i32) -> Hgdi;
        fn SelectObject(hdc: Hdc, obj: Hgdi) -> Hgdi;
        fn GetDIBits(
            hdc: Hdc,
            bmp: Hgdi,
            start: u32,
            lines: u32,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        fn DeleteObject(obj: Hgdi) -> i32;
        fn DeleteDC(hdc: Hdc) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: Hwnd,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }

    #[link(name = "shlwapi")]
    extern "system" {
        fn AssocQueryStringW(
            flags: u32,
            what: u32,
            assoc: *const u16,
            extra: *const u16,
            out: *mut u16,
            out_len: *mut u32,
        ) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    /// Window rects and PrintWindow must agree on pixels; without this a
    /// 125%-scaled screen yields cropped screenshots.
    pub fn set_dpi_aware() {
        const PER_MONITOR_AWARE_V2: isize = -4;
        unsafe {
            if SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2) == 0 {
                SetProcessDPIAware();
            }
        }
    }

    /// Open a file, folder or http(s) URL with its default program —
    /// ShellExecute directly, never through `cmd /C start` (cmd would treat
    /// `&`, `|`, `^` inside a received URL or file name as commands).
    pub fn shell_open(target: &str) -> anyhow::Result<()> {
        const SW_SHOWNORMAL: i32 = 1;
        let op = wide("open");
        let file = wide(target);
        let r = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if r <= 32 {
            anyhow::bail!("ShellExecute failed ({})", r);
        }
        Ok(())
    }

    /// Friendly name of the program Windows uses for `ext` (`docx`) or a
    /// URL scheme (`https`); None when nothing is registered.
    pub fn association(ext_or_scheme: &str) -> Option<String> {
        const ASSOCF_INIT_IGNOREUNKNOWN: u32 = 0x400;
        const ASSOCSTR_FRIENDLYAPPNAME: u32 = 4;
        let key = if ext_or_scheme.contains(':') || ext_or_scheme == "https" || ext_or_scheme == "http" {
            ext_or_scheme.to_string()
        } else {
            format!(".{}", ext_or_scheme)
        };
        let key = wide(&key);
        let verb = wide("open");
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let hr = unsafe {
            AssocQueryStringW(
                ASSOCF_INIT_IGNOREUNKNOWN,
                ASSOCSTR_FRIENDLYAPPNAME,
                key.as_ptr(),
                verb.as_ptr(),
                buf.as_mut_ptr(),
                &mut len,
            )
        };
        if hr != 0 || len <= 1 {
            return None;
        }
        let name = String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]);
        let name = name.trim().to_string();
        (!name.is_empty()).then_some(name)
    }

    pub fn foreground_window() -> usize {
        unsafe { GetForegroundWindow() as usize }
    }

    pub fn window_class(hwnd: usize) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(hwnd as Hwnd, buf.as_mut_ptr(), 256) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    /// JPEG of one window, even when another window (PalmSync's preview)
    /// covers it. None for minimised windows or when the app refuses to
    /// draw itself.
    pub fn capture_window_jpeg(hwnd: usize, max_w: u32, max_h: u32) -> Option<Vec<u8>> {
        const PW_RENDERFULLCONTENT: u32 = 2;
        const DIB_RGB_COLORS: u32 = 0;
        let hwnd = hwnd as Hwnd;
        unsafe {
            if hwnd.is_null() || IsIconic(hwnd) != 0 {
                return None;
            }
            let mut r = Rect::default();
            if GetWindowRect(hwnd, &mut r) == 0 {
                return None;
            }
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            if w < 16 || h < 16 || w > 16384 || h > 16384 {
                return None;
            }

            let screen = GetDC(std::ptr::null_mut());
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp);
            let drawn = PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT) != 0;

            let mut info = BitmapInfo {
                header: BitmapInfoHeader {
                    size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                    width: w,
                    height: -h, // top-down rows
                    planes: 1,
                    bit_count: 32,
                    compression: 0, // BI_RGB
                    size_image: 0,
                    x_ppm: 0,
                    y_ppm: 0,
                    clr_used: 0,
                    clr_important: 0,
                },
                colors: [0],
            };
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            let rows = if drawn {
                GetDIBits(
                    mem,
                    bmp,
                    0,
                    h as u32,
                    bgra.as_mut_ptr() as *mut c_void,
                    &mut info,
                    DIB_RGB_COLORS,
                )
            } else {
                0
            };

            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(std::ptr::null_mut(), screen);

            if rows != h {
                return None;
            }
            super::encode_jpeg(&bgra, w as u32, h as u32, max_w, max_h)
        }
    }

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        pid: u32,
        default_heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_pid: u32,
        pri_class_base: i32,
        flags: u32,
        exe_file: [u16; 260],
    }

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        max_length: u16,
        buffer: *const u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
        fn Process32FirstW(snap: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snap: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(h: *mut c_void) -> i32;
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn NtQueryInformationProcess(
            process: *mut c_void,
            class: u32,
            info: *mut c_void,
            len: u32,
            ret_len: *mut u32,
        ) -> i32;
    }

    fn command_line_of(pid: u32) -> Option<String> {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60; // Windows 8.1+
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            // u64 elements keep the UNICODE_STRING header aligned.
            let mut buf = vec![0u64; 1024];
            let mut needed = 0u32;
            let mut status = NtQueryInformationProcess(
                h,
                PROCESS_COMMAND_LINE_INFORMATION,
                buf.as_mut_ptr() as *mut c_void,
                (buf.len() * 8) as u32,
                &mut needed,
            );
            if status < 0 && needed as usize > buf.len() * 8 && needed < 1 << 20 {
                buf = vec![0u64; (needed as usize).div_ceil(8)];
                status = NtQueryInformationProcess(
                    h,
                    PROCESS_COMMAND_LINE_INFORMATION,
                    buf.as_mut_ptr() as *mut c_void,
                    (buf.len() * 8) as u32,
                    &mut needed,
                );
            }
            CloseHandle(h);
            if status < 0 {
                return None;
            }
            let us = &*(buf.as_ptr() as *const UnicodeString);
            if us.buffer.is_null() || us.length == 0 {
                return None;
            }
            let chars = std::slice::from_raw_parts(us.buffer, (us.length / 2) as usize);
            Some(String::from_utf16_lossy(chars))
        }
    }

    /// Command lines of running processes: `pid` first (if readable), then
    /// every other process whose command line passes `keep`. Native and
    /// fast (a few ms) — PowerShell/WMI took 130–360 ms per GRAB.
    pub fn process_command_lines(pid: u32, keep: &dyn Fn(&str) -> bool) -> Vec<String> {
        const TH32CS_SNAPPROCESS: u32 = 0x2;
        let mut out = Vec::new();
        if let Some(own) = command_line_of(pid) {
            out.push(own);
        }
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap.is_null() || snap as isize == -1 {
                return out;
            }
            let mut entry: ProcessEntry32W = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut entry);
            while ok != 0 {
                if entry.pid != pid && entry.pid != 0 {
                    if let Some(cmd) = command_line_of(entry.pid) {
                        if keep(&cmd) {
                            out.push(cmd);
                        }
                    }
                }
                ok = Process32NextW(snap, &mut entry);
            }
            CloseHandle(snap);
        }
        out
    }

    pub fn reveal_in_explorer(path: &Path) -> anyhow::Result<()> {
        use std::os::windows::process::CommandExt;
        // Our own received-files path (no quotes in sanitized names); the
        // folder is shown with the file selected, nothing is opened.
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()?;
        Ok(())
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
mod imp_stub {
    use std::path::Path;
    pub fn set_dpi_aware() {}
    pub fn shell_open(_target: &str) -> anyhow::Result<()> {
        anyhow::bail!("not supported on this platform")
    }
    pub fn association(_ext: &str) -> Option<String> {
        None
    }
    pub fn foreground_window() -> usize {
        0
    }
    pub fn window_class(_hwnd: usize) -> String {
        String::new()
    }
    pub fn capture_window_jpeg(_hwnd: usize, _w: u32, _h: u32) -> Option<Vec<u8>> {
        None
    }
    pub fn reveal_in_explorer(_path: &Path) -> anyhow::Result<()> {
        anyhow::bail!("not supported on this platform")
    }
    pub fn process_command_lines(_pid: u32, _keep: &dyn Fn(&str) -> bool) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(not(windows))]
pub use imp_stub::*;

/// Downscale (nearest neighbour is plenty for a "what was on screen"
/// preview) and encode BGRA pixels as a JPEG.
pub fn encode_jpeg(bgra: &[u8], w: u32, h: u32, max_w: u32, max_h: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || bgra.len() < (w * h * 4) as usize {
        return None;
    }
    let scale = (w as f64 / max_w as f64).max(h as f64 / max_h as f64).max(1.0);
    let (tw, th) = (
        ((w as f64 / scale) as u32).max(1),
        ((h as f64 / scale) as u32).max(1),
    );
    let mut rgb = Vec::with_capacity((tw * th * 3) as usize);
    for y in 0..th {
        let sy = ((y as f64 * scale) as u32).min(h - 1);
        for x in 0..tw {
            let sx = ((x as f64 * scale) as u32).min(w - 1);
            let i = ((sy * w + sx) * 4) as usize;
            rgb.extend_from_slice(&[bgra[i + 2], bgra[i + 1], bgra[i]]);
        }
    }
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, 72);
    encoder
        .encode(&rgb, tw as u16, th as u16, jpeg_encoder::ColorType::Rgb)
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn reads_command_lines_natively() {
        let me = std::process::id();
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_string_lossy().to_string();
        let started = std::time::Instant::now();
        let lines = process_command_lines(me, &|c| c.contains("no-such-marker-xyz"));
        println!("all process command lines read in {} ms", started.elapsed().as_millis());
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert!(!lines.is_empty() && lines[0].contains(&name), "{:?}", lines.first());
    }

    #[test]
    fn jpeg_is_downscaled_and_valid() {
        let (w, h) = (400u32, 200u32);
        let bgra: Vec<u8> = (0..w * h).flat_map(|i| [(i % 255) as u8, 80, 160, 255]).collect();
        let jpg = encode_jpeg(&bgra, w, h, 100, 100).unwrap();
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]); // SOI marker
        assert!(jpg.len() < 20_000);
        assert!(encode_jpeg(&bgra, w, h + 10, 100, 100).is_none()); // short buffer
    }
}
