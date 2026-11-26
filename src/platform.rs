//! OS-specific helpers: Windows Administrator elevation check, relaunch-as-Admin,
//! Windows Job Object guard (so child VPN processes never leak on exit/crash),
//! binary path detection, and process-tree termination.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows creation flag to hide console windows for child CLI processes.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Configures a `Command` so it does not flash a console window on Windows.
pub fn configure_background_command(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Detects whether the current process is running with elevated (Administrator / root) privileges.
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        use std::mem;
        use std::ptr;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token: HANDLE = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut returned_len: u32 = 0;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned_len,
            );
            CloseHandle(token);
            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    #[cfg(not(windows))]
    {
        std::env::var("USER").is_ok_and(|u| u == "root")
    }
}

/// Requests Windows UAC elevation by relaunching the current executable via `ShellExecuteW("runas")`.
pub fn relaunch_as_admin() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::ptr;
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let exe = std::env::current_exe()
            .map_err(|e| format!("Could not resolve current executable: {e}"))?;
        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let file: Vec<u16> = exe
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if (result as isize) <= 32 {
            return Err("Administrator elevation was cancelled or failed.".to_string());
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        Err("Interactive UAC relaunch is only supported on Windows.".to_string())
    }
}

/// Terminates a child process and its entire process tree cleanly.
pub fn terminate_child_tree(child: &mut Child) {
    let pid = child.id();

    #[cfg(windows)]
    {
        let mut kill_cmd = Command::new("taskkill");
        kill_cmd
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_background_command(&mut kill_cmd);
        let _ = kill_cmd.status();
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = pid;
}

/// Owns a Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
/// so all assigned child VPN processes are killed automatically by the OS if Kraken VPN exits.
pub struct ChildJobGuard {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for ChildJobGuard {}
#[cfg(windows)]
unsafe impl Sync for ChildJobGuard {}

impl ChildJobGuard {
    pub fn new() -> Option<Self> {
        #[cfg(windows)]
        {
            use std::mem;
            use std::ptr;
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::JobObjects::{
                CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };

            unsafe {
                let handle = CreateJobObjectW(ptr::null(), ptr::null());
                if handle.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    CloseHandle(handle);
                    return None;
                }
                Some(Self { handle })
            }
        }
        #[cfg(not(windows))]
        {
            Some(Self {})
        }
    }

    pub fn assign(&self, child: &Child) {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

            let proc_handle = child.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
            unsafe {
                let _ = AssignProcessToJobObject(self.handle, proc_handle);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
        }
    }
}

#[cfg(windows)]
impl Drop for ChildJobGuard {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[derive(Debug, Clone)]
pub struct BinaryCheckResult {
    pub found: bool,
    pub resolved_path: Option<PathBuf>,
    pub version_line: Option<String>,
    pub error: Option<String>,
}

/// Resolves an executable path (either explicit path or lookup on `PATH`).
pub fn resolve_binary(configured: &str) -> Option<PathBuf> {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() || trimmed.contains(std::path::MAIN_SEPARATOR) || trimmed.contains('/') {
        return path.is_file().then(|| path.to_path_buf());
    }

    // Check PATH environment variable
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
            #[cfg(windows)]
            if path.extension().is_none() {
                let with_exe = dir.join(format!("{trimmed}.exe"));
                if with_exe.is_file() {
                    return Some(with_exe);
                }
            }
        }
    }

    // Check standard Windows install locations when bare executable name isn't on PATH
    #[cfg(windows)]
    {
        let lower = trimmed.to_ascii_lowercase();
        if lower == "openconnect" || lower == "openconnect.exe" {
            for candidate in [
                r"C:\Program Files\OpenConnect-GUI\openconnect.exe",
                r"C:\Program Files (x86)\OpenConnect-GUI\openconnect.exe",
            ] {
                let p = Path::new(candidate);
                if p.is_file() {
                    return Some(p.to_path_buf());
                }
            }
        } else if lower == "openvpn" || lower == "openvpn.exe" {
            for candidate in [
                r"C:\Program Files\OpenVPN\bin\openvpn.exe",
                r"C:\Program Files (x86)\OpenVPN\bin\openvpn.exe",
            ] {
                let p = Path::new(candidate);
                if p.is_file() {
                    return Some(p.to_path_buf());
                }
            }
        }
    }

    None
}

/// Tests whether a VPN binary exists and runs `--version`.
pub fn check_binary(configured: &str) -> BinaryCheckResult {
    let Some(resolved) = resolve_binary(configured) else {
        return BinaryCheckResult {
            found: false,
            resolved_path: None,
            version_line: None,
            error: Some(format!("Binary not found: '{configured}'")),
        };
    };

    let mut cmd = Command::new(&resolved);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_background_command(&mut cmd);

    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let first = stdout
                .lines()
                .chain(stderr.lines())
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string);
            BinaryCheckResult {
                found: true,
                resolved_path: Some(resolved),
                version_line: first,
                error: None,
            }
        }
        Err(e) => BinaryCheckResult {
            found: false,
            resolved_path: Some(resolved),
            version_line: None,
            error: Some(format!("Failed to execute binary: {e}")),
        },
    }
}
