//! Native Windows startup keeps the console hidden and suspends the initial thread
//! until its process has been placed in a kill-on-close Job Object.
use crate::{model::Preview, process::run_marker};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::File,
    mem::{size_of, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
        process::ExitStatusExt,
    },
    process::ExitStatus,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::*,
    System::{Pipes::*, Threading::*},
};

type Result<T> = std::result::Result<T, String>;
fn error() -> String {
    std::io::Error::last_os_error().to_string()
}
fn wide(text: &std::ffi::OsStr) -> Vec<u16> {
    text.encode_wide().chain(Some(0)).collect()
}
fn handle(raw: HANDLE) -> OwnedHandle {
    unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) }
}

pub struct WindowsChild {
    process: OwnedHandle,
    thread: Option<OwnedHandle>,
    pid: u32,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
}
impl AsRawHandle for WindowsChild {
    fn as_raw_handle(&self) -> RawHandle {
        self.process.as_raw_handle()
    }
}
impl WindowsChild {
    pub fn id(&self) -> u32 {
        self.pid
    }
    pub fn resume(&mut self) -> Result<()> {
        let thread = self.thread.take().ok_or("初始线程已恢复")?;
        if unsafe { ResumeThread(thread.as_raw_handle() as _) } == u32::MAX {
            return Err(error());
        }
        Ok(())
    }
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        let result = unsafe { WaitForSingleObject(self.as_raw_handle() as _, 0) };
        if result == WAIT_TIMEOUT {
            return Ok(None);
        }
        if result != WAIT_OBJECT_0 {
            return Err(error());
        }
        let mut code = 0;
        if unsafe { GetExitCodeProcess(self.as_raw_handle() as _, &mut code) } == 0 {
            return Err(error());
        }
        Ok(Some(ExitStatus::from_raw(code)))
    }
    pub fn wait(&mut self) -> Result<ExitStatus> {
        if unsafe { WaitForSingleObject(self.as_raw_handle() as _, INFINITE) } != WAIT_OBJECT_0 {
            return Err(error());
        }
        self.try_wait()?.ok_or("进程尚未结束".into())
    }
    pub fn spawn(preview: &Preview, run_id: &str, job: HANDLE) -> Result<Self> {
        let attrs = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let pipe = || -> Result<(File, OwnedHandle)> {
            let (mut read, mut write) = (null_mut(), null_mut());
            if unsafe { CreatePipe(&mut read, &mut write, &attrs, 0) } == 0 {
                return Err(error());
            }
            let read = handle(read);
            let write = handle(write);
            if unsafe { SetHandleInformation(read.as_raw_handle() as _, HANDLE_FLAG_INHERIT, 0) }
                == 0
            {
                return Err(error());
            }
            Ok((File::from(read), write))
        };
        let (stdout, stdout_write) = pipe()?;
        let (stderr, stderr_write) = pipe()?;
        let nul: Vec<_> = "NUL".encode_utf16().chain(Some(0)).collect();
        let input = unsafe {
            CreateFileW(
                nul.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &attrs,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if input == INVALID_HANDLE_VALUE {
            return Err(error());
        }
        let input = handle(input);
        let handles = [
            input.as_raw_handle(),
            stdout_write.as_raw_handle(),
            stderr_write.as_raw_handle(),
        ];
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(error());
        }
        // usize backing storage provides the alignment required by the opaque list.
        let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        if unsafe { InitializeProcThreadAttributeList(list, 2, 0, &mut bytes) } == 0 {
            return Err(error());
        }
        struct Attributes(LPPROC_THREAD_ATTRIBUTE_LIST);
        impl Drop for Attributes {
            fn drop(&mut self) {
                unsafe {
                    DeleteProcThreadAttributeList(self.0);
                }
            }
        }
        let _attributes = Attributes(list);
        if unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr() as _,
                size_of::<[RawHandle; 3]>(),
                null_mut(),
                null(),
            )
        } == 0
        {
            return Err(error());
        }
        // Assign at creation, eliminating the crash window between spawn and assignment.
        if unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                &job as *const _ as _,
                size_of::<HANDLE>(),
                null_mut(),
                null(),
            )
        } == 0
        {
            return Err(error());
        }
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
        startup.StartupInfo.wShowWindow = 0; // SW_HIDE
        startup.StartupInfo.hStdInput = input.as_raw_handle() as _;
        startup.StartupInfo.hStdOutput = stdout_write.as_raw_handle() as _;
        startup.StartupInfo.hStdError = stderr_write.as_raw_handle() as _;
        startup.lpAttributeList = list;
        let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| OsString::from("cmd.exe"));
        let application = wide(&shell);
        let mut command = wide(std::ffi::OsStr::new(&format!(
            "\"{}\" /D /S /C \"{}\"",
            shell.to_string_lossy(),
            preview.command
        )));
        let directory = wide(std::ffi::OsStr::new(&preview.working_directory));
        // Windows environment names are case-insensitive. Preserve OS values and
        // apply task overrides using that same comparison.
        let mut environment: BTreeMap<String, (OsString, OsString)> = std::env::vars_os()
            .map(|(k, v)| (k.to_string_lossy().to_uppercase(), (k, v)))
            .collect();
        for (k, v) in &preview.environment {
            environment.insert(k.to_uppercase(), (k.into(), v.into()));
        }
        let marker = run_marker(run_id);
        environment.insert(marker.to_uppercase(), (marker.into(), "1".into()));
        let mut block: Vec<u16> = Vec::new();
        for (_, (key, value)) in environment {
            block.extend(key.encode_wide());
            block.push('=' as u16);
            block.extend(value.encode_wide());
            block.push(0);
        }
        block.push(0);
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        let ok = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_NEW_CONSOLE
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                block.as_ptr() as _,
                directory.as_ptr(),
                &startup.StartupInfo,
                &mut info,
            )
        };
        if ok == 0 {
            return Err(format!(
                "启动失败：{}。请检查程序路径、工作目录和运行环境。",
                error()
            ));
        }
        Ok(Self {
            process: handle(info.hProcess),
            thread: Some(handle(info.hThread)),
            pid: info.dwProcessId,
            stdout: Some(stdout),
            stderr: Some(stderr),
        })
    }
}
