//! Per-user Windows "Open with" and Default Apps file association registration.
//!
//! Writes under `HKCU\Software\Classes` and `HKCU\Software\RegisteredApplications`
//! (no administrator privileges required) and notifies Windows Explorer via
//! `SHChangeNotify(SHCNE_ASSOCCHANGED)` so Peeklet appears in the right-click
//! "Open with" menu and Windows Default Apps immediately.

/// File extensions that Peeklet registers for Windows "Open with".
pub const OPEN_WITH_EXTENSIONS: &[&str] = &[
    "txt", "md", "markdown", "rs", "toml", "json", "yaml", "yml", "xml", "html", "css", "js", "ts",
    "jsx", "tsx", "py", "go", "java", "c", "h", "cpp", "hpp", "sql", "ini", "conf", "csv", "tsv",
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "ico", "svg", "pdf",
];

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};

    use super::OPEN_WITH_EXTENSIONS;

    type Hkey = *mut c_void;
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001usize as Hkey;
    const KEY_READ: u32 = 0x0002_0019;
    const KEY_WRITE: u32 = 0x0002_0006;
    const REG_SZ: u32 = 1;
    const ERROR_SUCCESS: i32 = 0;
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0x0000;

    const PROG_ID: &str = "Peeklet.File";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegCreateKeyExW(
            hkey: Hkey,
            sub_key: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            sam_desired: u32,
            security_attributes: *const c_void,
            result: *mut Hkey,
            disposition: *mut u32,
        ) -> i32;
        fn RegOpenKeyExW(
            hkey: Hkey,
            sub_key: *const u16,
            options: u32,
            sam_desired: u32,
            result: *mut Hkey,
        ) -> i32;
        fn RegSetValueExW(
            hkey: Hkey,
            value_name: *const u16,
            reserved: u32,
            value_type: u32,
            data: *const u8,
            cb_data: u32,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: Hkey,
            value_name: *const u16,
            reserved: *const u32,
            value_type: *mut u32,
            data: *mut u8,
            cb_data: *mut u32,
        ) -> i32;
        fn RegDeleteValueW(hkey: Hkey, value_name: *const u16) -> i32;
        fn RegDeleteTreeW(hkey: Hkey, sub_key: *const u16) -> i32;
        fn RegCloseKey(hkey: Hkey) -> i32;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(event_id: i32, flags: u32, item1: *const c_void, item2: *const c_void);
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn current_exe_clean() -> Result<(String, String), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Could not resolve Peeklet executable path: {e}"))?;
        let raw = exe.to_string_lossy().into_owned();
        let clean = raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string();
        let exe_name = exe
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "peeklet.exe".to_string());
        Ok((clean, exe_name))
    }

    fn set_sz(sub_key: &str, value_name: Option<&str>, value: &str) -> Result<(), String> {
        let sub_w = to_wide(sub_key);
        let mut hkey: Hkey = null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub_w.as_ptr(),
                0,
                null(),
                0,
                KEY_WRITE,
                null(),
                &mut hkey,
                null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegCreateKeyExW({sub_key}) failed: {status}"));
        }
        let val_w = to_wide(value);
        let name_w = value_name.map(to_wide);
        let name_ptr = name_w.as_ref().map(|v| v.as_ptr()).unwrap_or(null());
        let set_status = unsafe {
            RegSetValueExW(
                hkey,
                name_ptr,
                0,
                REG_SZ,
                val_w.as_ptr() as *const u8,
                (val_w.len() * 2) as u32,
            )
        };
        unsafe { RegCloseKey(hkey) };
        if set_status != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW({sub_key}) failed: {set_status}"));
        }
        Ok(())
    }

    fn query_sz(sub_key: &str, value_name: Option<&str>) -> Option<String> {
        let sub_w = to_wide(sub_key);
        let mut hkey: Hkey = null_mut();
        let status = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, sub_w.as_ptr(), 0, KEY_READ, &mut hkey)
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let name_w = value_name.map(to_wide);
        let name_ptr = name_w.as_ref().map(|v| v.as_ptr()).unwrap_or(null());
        let mut cb: u32 = 0;
        let mut ty: u32 = 0;
        let q1 = unsafe {
            RegQueryValueExW(hkey, name_ptr, null(), &mut ty, null_mut(), &mut cb)
        };
        if q1 != ERROR_SUCCESS || ty != REG_SZ || cb == 0 {
            unsafe { RegCloseKey(hkey) };
            return None;
        }
        let mut buf: Vec<u16> = vec![0; (cb as usize + 1) / 2];
        let q2 = unsafe {
            RegQueryValueExW(
                hkey,
                name_ptr,
                null(),
                &mut ty,
                buf.as_mut_ptr() as *mut u8,
                &mut cb,
            )
        };
        unsafe { RegCloseKey(hkey) };
        if q2 != ERROR_SUCCESS {
            return None;
        }
        while buf.last() == Some(&0) {
            buf.pop();
        }
        Some(String::from_utf16_lossy(&buf))
    }

    fn delete_tree(sub_key: &str) {
        let sub_w = to_wide(sub_key);
        unsafe {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, sub_w.as_ptr());
        }
    }

    fn delete_value(sub_key: &str, value_name: &str) {
        let sub_w = to_wide(sub_key);
        let mut hkey: Hkey = null_mut();
        let status = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, sub_w.as_ptr(), 0, KEY_WRITE, &mut hkey)
        };
        if status == ERROR_SUCCESS {
            let name_w = to_wide(value_name);
            unsafe {
                let _ = RegDeleteValueW(hkey, name_w.as_ptr());
                RegCloseKey(hkey);
            }
        }
    }

    fn notify_shell() {
        unsafe {
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, null(), null());
        }
    }

    pub fn register_windows_open_with() -> Result<(), String> {
        let (exe, exe_name) = current_exe_clean()?;
        let open_cmd = format!("\"{exe}\" \"%1\"");
        let icon_val = format!("\"{exe}\",0");

        // 1. HKCU\Software\Classes\Applications\<exe_name> (Explorer "Open with")
        let app_key = format!(r"Software\Classes\Applications\{exe_name}");
        set_sz(&app_key, Some("FriendlyAppName"), "Peeklet")?;
        set_sz(&format!(r"{app_key}\DefaultIcon"), None, &icon_val)?;
        set_sz(&format!(r"{app_key}\shell\open\command"), None, &open_cmd)?;
        let supported_key = format!(r"{app_key}\SupportedTypes");
        for ext in OPEN_WITH_EXTENSIONS {
            set_sz(&supported_key, Some(&format!(".{ext}")), "")?;
        }

        // 2. HKCU\Software\Classes\Peeklet.File (ProgID)
        let prog_key = format!(r"Software\Classes\{PROG_ID}");
        set_sz(&prog_key, None, "Peeklet File")?;
        set_sz(&format!(r"{prog_key}\DefaultIcon"), None, &icon_val)?;
        set_sz(&format!(r"{prog_key}\shell\open\command"), None, &open_cmd)?;

        // 3. HKCU\Software\Classes\.<ext>\OpenWithProgids -> Peeklet.File
        for ext in OPEN_WITH_EXTENSIONS {
            let ext_key = format!(r"Software\Classes\.{ext}\OpenWithProgids");
            set_sz(&ext_key, Some(PROG_ID), "")?;
        }

        // 4. Windows Settings -> Default Apps Capabilities
        set_sz(
            r"Software\Peeklet\Capabilities",
            Some("ApplicationName"),
            "Peeklet",
        )?;
        set_sz(
            r"Software\Peeklet\Capabilities",
            Some("ApplicationDescription"),
            "Fast lightweight viewer and editor for images, Markdown, SVG, PDF, and code.",
        )?;
        for ext in OPEN_WITH_EXTENSIONS {
            set_sz(
                r"Software\Peeklet\Capabilities\FileAssociations",
                Some(&format!(".{ext}")),
                PROG_ID,
            )?;
        }
        set_sz(
            r"Software\RegisteredApplications",
            Some("Peeklet"),
            r"Software\Peeklet\Capabilities",
        )?;

        notify_shell();
        Ok(())
    }

    pub fn unregister_windows_open_with() -> Result<(), String> {
        let (_, exe_name) = current_exe_clean()?;
        delete_tree(&format!(r"Software\Classes\Applications\{exe_name}"));
        delete_tree(&format!(r"Software\Classes\{PROG_ID}"));
        delete_tree(r"Software\Peeklet");
        delete_value(r"Software\RegisteredApplications", "Peeklet");
        for ext in OPEN_WITH_EXTENSIONS {
            delete_value(&format!(r"Software\Classes\.{ext}\OpenWithProgids"), PROG_ID);
        }
        notify_shell();
        Ok(())
    }

    pub fn is_registered() -> bool {
        let Ok((exe, _)) = current_exe_clean() else {
            return false;
        };
        let expected = format!("\"{exe}\" \"%1\"");
        query_sz(&format!(r"Software\Classes\{PROG_ID}\shell\open\command"), None)
            .is_some_and(|v| v.eq_ignore_ascii_case(&expected))
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn register_windows_open_with() -> Result<(), String> {
        Ok(())
    }
    pub fn unregister_windows_open_with() -> Result<(), String> {
        Ok(())
    }
    pub fn is_registered() -> bool {
        false
    }
}

pub use imp::{is_registered, register_windows_open_with, unregister_windows_open_with};
