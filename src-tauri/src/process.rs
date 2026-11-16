use crate::model::Preview;
#[cfg(windows)]
use crate::windows_process::WindowsChild as Child;
#[cfg(unix)]
use std::process::Child;
use std::process::{Command, Stdio};

pub fn run_marker(run_id: &str) -> String {
    format!("SCRIPT_HUB_RUN_{}", run_id.replace('-', "_"))
}

pub struct Process {
    pub child: Child,
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
}

// The worker exclusively owns the process/job handles.
#[cfg(windows)]
unsafe impl Send for Process {}

impl Process {
    pub fn spawn(preview: &Preview, run_id: &str) -> Result<Self, String> {
        #[cfg(not(windows))]
        let mut command = {
            use std::os::unix::process::CommandExt;
            let mut c = Command::new("/bin/sh");
            c.arg("-c").arg(&preview.command);
            unsafe {
                c.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            c
        };
        #[cfg(unix)]
        let child = command
            .current_dir(&preview.working_directory)
            .envs(&preview.environment)
            .env(run_marker(run_id), "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("启动失败：{e}。请检查程序路径、工作目录和运行环境。"))?;
        #[cfg(windows)]
        {
            use std::mem::{size_of, zeroed};
            use windows_sys::Win32::{Foundation::*, System::JobObjects::*};
            let name: Vec<u16> = format!("Local\\ScriptHub-{run_id}")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), name.as_ptr());
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let assigned = !job.is_null()
                    && SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        &limits as *const _ as _,
                        size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    ) != 0;
                if !assigned {
                    let err = std::io::Error::last_os_error();
                    if !job.is_null() {
                        CloseHandle(job);
                    }
                    return Err(format!("无法创建受管进程树：{err}"));
                }
                let mut child = match Child::spawn(preview, run_id, job) {
                    Ok(child) => child,
                    Err(e) => {
                        CloseHandle(job);
                        return Err(e);
                    }
                };
                if let Err(e) = child.resume() {
                    TerminateJobObject(job, 1);
                    CloseHandle(job);
                    return Err(e);
                }
                Ok(Self { child, job })
            }
        }
        #[cfg(not(windows))]
        {
            Ok(Self { child })
        }
    }

    pub fn graceful(&self) -> Result<(), String> {
        #[cfg(unix)]
        {
            if unsafe { libc::kill(-(self.child.id() as i32), libc::SIGTERM) } == -1 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() != Some(libc::ESRCH) {
                    return Err(e.to_string());
                }
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            // A short-lived helper attaches to this run's private console, so signals
            // for concurrent tasks cannot race through the GUI app's global console.
            let mut signal = console_helper(self.child.id(), "stop");
            if signal.is_err()
                && let Ok(pids) = self.process_ids()
            {
                for pid in pids {
                    if pid != self.child.id() && console_helper(pid, "stop").is_ok() {
                        signal = Ok(());
                        break;
                    }
                }
            }
            use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};
            unsafe extern "system" fn close(window: HWND, job: isize) -> i32 {
                use windows_sys::Win32::{
                    Foundation::CloseHandle,
                    System::{JobObjects::IsProcessInJob, Threading::*},
                };
                let mut pid = 0;
                unsafe {
                    GetWindowThreadProcessId(window, &mut pid);
                    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                    if !process.is_null() {
                        let mut inside = 0;
                        if IsProcessInJob(process, job as _, &mut inside) != 0 && inside != 0 {
                            PostMessageW(window, 0x0010, 0, 0);
                        }
                        CloseHandle(process);
                    }
                }
                1
            }
            unsafe {
                EnumWindows(Some(close), self.job as isize);
            }
            signal
        }
    }

    #[cfg(windows)]
    fn process_ids(&self) -> Result<Vec<u32>, String> {
        use windows_sys::Win32::{
            Foundation::{ERROR_MORE_DATA, GetLastError},
            System::JobObjects::*,
        };
        let mut buffer = vec![0usize; 128];
        loop {
            let ok = unsafe {
                QueryInformationJobObject(
                    self.job,
                    JobObjectBasicProcessIdList,
                    buffer.as_mut_ptr() as _,
                    std::mem::size_of_val(buffer.as_slice()) as u32,
                    std::ptr::null_mut(),
                )
            };
            if ok != 0 {
                let info = buffer.as_ptr() as *const JOBOBJECT_BASIC_PROCESS_ID_LIST;
                unsafe {
                    let len = (*info).NumberOfProcessIdsInList as usize;
                    return Ok(
                        std::slice::from_raw_parts((*info).ProcessIdList.as_ptr(), len)
                            .iter()
                            .map(|p| *p as u32)
                            .collect(),
                    );
                }
            }
            if unsafe { GetLastError() } != ERROR_MORE_DATA {
                return Err(std::io::Error::last_os_error().to_string());
            }
            buffer.resize(buffer.len() * 2, 0);
        }
    }

    pub fn force(&self) -> Result<(), String> {
        #[cfg(unix)]
        {
            if unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) } == -1 {
                let e = std::io::Error::last_os_error();
                if e.raw_os_error() != Some(libc::ESRCH) {
                    return Err(e.to_string());
                }
            }
            Ok(())
        }
        #[cfg(windows)]
        {
            if unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1) }
                == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(())
        }
    }
    pub fn tree_alive(&self) -> Result<bool, String> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::JobObjects::*;
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
            let ok = unsafe {
                QueryInformationJobObject(
                    self.job,
                    JobObjectBasicAccountingInformation,
                    &mut info as *mut _ as _,
                    std::mem::size_of_val(&info) as u32,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(info.ActiveProcesses > 0)
        }
        #[cfg(unix)]
        {
            group_alive(self.child.id())
        }
    }
}

#[cfg(windows)]
fn console_helper(pid: u32, action: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    #[cfg(not(test))]
    command.args(["--script-hub-console", &pid.to_string(), action]);
    #[cfg(test)]
    command
        .args([
            "--exact",
            "process::windows_tests::console_helper_entry",
            "--ignored",
        ])
        .env("SCRIPT_HUB_TEST_CONSOLE_PID", pid.to_string())
        .env("SCRIPT_HUB_TEST_CONSOLE_ACTION", action);
    let mut helper = command
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Some(status) = helper.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err("控制台请求未送达；停止超时后将强制结束进程树".into())
            };
        }
        if std::time::Instant::now() > deadline {
            let _ = helper.kill();
            let _ = helper.wait();
            return Err("控制台请求超时".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(windows)]
pub fn handle_console_helper() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--script-hub-console") {
        return false;
    }
    let Some(pid) = args.get(2).and_then(|s| s.parse::<u32>().ok()) else {
        std::process::exit(2);
    };
    std::process::exit(
        if perform_console_request(pid, args.get(3).map(String::as_str).unwrap_or("stop")) {
            0
        } else {
            1
        },
    );
}

#[cfg(windows)]
fn perform_console_request(pid: u32, action: &str) -> bool {
    use windows_sys::Win32::{System::Console::*, UI::WindowsAndMessaging::*};
    unsafe extern "system" fn ignore(_: u32) -> i32 {
        1
    }
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            false
        } else {
            SetConsoleCtrlHandler(Some(ignore), 1);
            let ok = if action == "hide" {
                let window = GetConsoleWindow();
                if !window.is_null() {
                    ShowWindow(window, SW_HIDE);
                }
                true
            } else {
                GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, 0) != 0
            };
            FreeConsole();
            ok
        }
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    #[test]
    #[ignore = "only invoked as a private console helper by process tests"]
    fn console_helper_entry() {
        let Ok(pid) = std::env::var("SCRIPT_HUB_TEST_CONSOLE_PID") else {
            return;
        };
        let action = std::env::var("SCRIPT_HUB_TEST_CONSOLE_ACTION").unwrap();
        std::process::exit(
            if super::perform_console_request(pid.parse().unwrap(), &action) {
                0
            } else {
                1
            },
        );
    }
}

#[cfg(windows)]
impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(unix)]
fn group_alive(pid: u32) -> Result<bool, String> {
    // Linux kill(0) includes zombies, which cannot execute and may be adopted by a
    // container's non-reaping PID 1. Inspect procfs to distinguish them.
    #[cfg(target_os = "linux")]
    {
        for entry in std::fs::read_dir("/proc")
            .map_err(|e| e.to_string())?
            .flatten()
        {
            if let Ok(stat) = std::fs::read_to_string(entry.path().join("stat"))
                && let Some((_, tail)) = stat.rsplit_once(") ")
            {
                let fields: Vec<_> = tail.split_whitespace().collect();
                if fields.len() > 2 && fields[2].parse::<u32>() == Ok(pid) && fields[0] != "Z" {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    #[cfg(not(target_os = "linux"))]
    {
        if unsafe { libc::kill(-(pid as i32), 0) } == 0 {
            return Ok(true);
        }
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() == Some(libc::ESRCH) {
            Ok(false)
        } else {
            Err(e.to_string())
        }
    }
}

pub fn orphan_alive(pid: u32, run_id: &str) -> Result<bool, String> {
    #[cfg(windows)]
    let _ = pid;
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::*, System::JobObjects::*};
        let name: Vec<_> = format!("Local\\ScriptHub-{run_id}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let job = OpenJobObjectW(
                windows_sys::Win32::System::SystemServices::JOB_OBJECT_QUERY,
                0,
                name.as_ptr(),
            );
            if job.is_null() {
                if GetLastError() == ERROR_FILE_NOT_FOUND {
                    return Ok(false);
                }
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
            let ok = QueryInformationJobObject(
                job,
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as _,
                std::mem::size_of_val(&info) as u32,
                std::ptr::null_mut(),
            );
            CloseHandle(job);
            if ok == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(info.ActiveProcesses > 0)
        }
    }
    #[cfg(unix)]
    {
        let _ = run_id;
        group_alive(pid)
    }
}

pub fn locate_orphan(pid: Option<u32>, run_id: &str) -> Result<Option<u32>, String> {
    if let Some(pid) = pid {
        return orphan_alive(pid, run_id).map(|alive| alive.then_some(pid));
    }
    #[cfg(windows)]
    {
        orphan_alive(0, run_id).map(|alive| alive.then_some(0))
    }
    #[cfg(target_os = "linux")]
    {
        let expected = format!("{}=1", run_marker(run_id));
        for entry in std::fs::read_dir("/proc")
            .map_err(|e| e.to_string())?
            .flatten()
        {
            if let Ok(env) = std::fs::read(entry.path().join("environ"))
                && env.split(|b| *b == 0).any(|v| v == expected.as_bytes())
                && let Ok(stat) = std::fs::read_to_string(entry.path().join("stat"))
                && let Some((_, tail)) = stat.rsplit_once(") ")
                && let Some(group) = tail
                    .split_whitespace()
                    .nth(2)
                    .and_then(|v| v.parse::<u32>().ok())
            {
                return Ok(Some(group));
            }
        }
        Ok(None)
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        Err("上次启动中断且未保存进程身份，请检查遗留运行实例".into())
    }
}

pub fn cleanup_orphan(pid: u32, run_id: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::*, System::JobObjects::*};
        let name: Vec<_> = format!("Local\\ScriptHub-{run_id}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let job = OpenJobObjectW(
                windows_sys::Win32::System::SystemServices::JOB_OBJECT_TERMINATE,
                0,
                name.as_ptr(),
            );
            if job.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let result = TerminateJobObject(job, 1);
            CloseHandle(job);
            if result == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        let _ = pid;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        let expected = format!("{}=1", run_marker(run_id));
        let mut verified = false;
        for entry in std::fs::read_dir("/proc")
            .map_err(|e| e.to_string())?
            .flatten()
        {
            if let Ok(stat) = std::fs::read_to_string(entry.path().join("stat"))
                && let Some((_, tail)) = stat.rsplit_once(") ")
            {
                let fields: Vec<_> = tail.split_whitespace().collect();
                if fields.get(2).and_then(|v| v.parse::<u32>().ok()) == Some(pid)
                    && let Ok(env) = std::fs::read(entry.path().join("environ"))
                    && env.split(|b| *b == 0).any(|v| v == expected.as_bytes())
                {
                    verified = true;
                }
            }
        }
        if !group_alive(pid)? {
            return Ok(());
        }
        if !verified {
            return Err("无法确认遗留进程身份。请检查并手动结束后重新核查。".into());
        }
        if unsafe { libc::kill(-(pid as i32), libc::SIGKILL) } == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = (pid, run_id);
        Err("请手动清理遗留进程后重新核查".into())
    }
}
