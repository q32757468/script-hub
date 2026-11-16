use crate::{
    model::*,
    process::{self, Process},
    storage::{Store, atomic_copy, atomic_write},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_LOG: usize = 10 * 1024 * 1024;
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub snapshot: Task,
    pub preview: PreviewRecord,
    pub status: String,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub pid: Option<u32>,
    pub message: Option<String>,
    #[serde(default)]
    pub truncated: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRecord {
    pub command: String,
    pub working_directory: String,
    pub environment: std::collections::BTreeMap<String, String>,
}
impl From<&Preview> for PreviewRecord {
    fn from(p: &Preview) -> Self {
        Self {
            command: p.command.clone(),
            working_directory: p.working_directory.clone(),
            environment: p.environment.clone(),
        }
    }
}
pub fn active(status: &str) -> bool {
    matches!(status, "starting" | "running" | "stopping" | "abnormal")
}

struct Control {
    stop: AtomicBool,
}
pub struct Hub {
    pub store: Store,
    pub config: Config,
    pub config_error: Option<String>,
    pub runtime_error: Option<String>,
    pub app_error: Option<String>,
    pub runs: Vec<Run>,
    runtime_loaded: bool,
    controls: HashMap<String, Arc<Control>>,
    pub shutting_down: bool,
}
#[derive(Clone)]
pub struct Shared(pub Arc<Mutex<Hub>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    pub config_error: Option<String>,
    pub runtime_error: Option<String>,
    pub app_error: Option<String>,
    pub root: String,
    pub runs: Vec<Run>,
}

impl Hub {
    pub fn open(root: PathBuf) -> Result<Self> {
        let mut store = Store::new(root)?;
        let (config, config_error) = match store.load() {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(e)),
        };
        let (runs, runtime_error) = match fs::read(store.root.join("runs.json")) {
            Ok(v) => match serde_json::from_slice::<Vec<Run>>(&v) {
                Ok(runs) if valid_runs(&runs) => (runs, None),
                _ => (vec![], Some("运行状态文件损坏。为防止重复执行，已阻止启动；请检查 runs.json 和遗留进程后重启应用。".into())),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (vec![], None),
            Err(e) => (vec![], Some(format!("无法读取运行状态：{e}"))),
        };
        let runtime_loaded = runtime_error.is_none();
        let mut hub = Self {
            store,
            config,
            config_error,
            runtime_error,
            app_error: None,
            runs,
            runtime_loaded,
            controls: HashMap::new(),
            shutting_down: false,
        };
        for run in &mut hub.runs {
            if active(&run.status) {
                match process::locate_orphan(run.pid, &run.id) {
                    Ok(None) => {
                        run.status = "interrupted".into();
                        run.ended_at = Some(now());
                        run.message = Some("上次软件异常退出，旧运行实例已结束".into());
                    }
                    other => {
                        if let Ok(Some(pid)) = &other {
                            run.pid = Some(*pid);
                        }
                        run.status = "abnormal".into();
                        run.message = Some(format!(
                            "可能存在遗留进程，请清理并核查后再运行。{}",
                            other.err().unwrap_or_default()
                        ));
                    }
                }
            }
        }
        if hub.runtime_error.is_none() {
            hub.persist()?;
        }
        Ok(hub)
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            config: self.config.clone(),
            config_error: self.config_error.clone(),
            runtime_error: self.runtime_error.clone(),
            app_error: self.app_error.clone(),
            root: self.store.root.to_string_lossy().into(),
            runs: self.runs.clone(),
        }
    }
    pub fn persist(&mut self) -> Result<()> {
        if !self.runtime_loaded {
            return Err("运行状态未有效加载，保留原文件；请检查后重启应用".into());
        }
        let result = atomic_write(
            &self.store.root.join("runs.json"),
            &serde_json::to_vec_pretty(&self.runs).map_err(|e| e.to_string())?,
        );
        if let Err(e) = &result {
            self.runtime_error = Some(format!("运行状态保存失败，已阻止新启动：{e}"));
        }
        result
    }
    pub fn require_stopped(&self) -> Result<()> {
        if self.runs.iter().any(|r| active(&r.status)) {
            return Err("请先停止全部运行实例并处理异常状态".into());
        }
        Ok(())
    }
    pub fn save(&mut self, config: Config, overwrite: bool) -> Result<()> {
        if let Some(e) = &self.config_error {
            return Err(e.clone());
        }
        for run in &self.runs {
            if active(&run.status) {
                if !config.tasks.iter().any(|t| t.id == run.task_id) {
                    return Err("运行中的任务必须先停止才能删除".into());
                }
                if let Command::Composed { parts } = &run.snapshot.command {
                    for part in parts {
                        if let Part::File { file_id } = part
                            && !config.files.iter().any(|f| &f.id == file_id)
                        {
                            return Err("运行实例仍引用该文件，请先停止任务".into());
                        }
                    }
                }
            }
        }
        // File identity and storage names are managed only through import/replace/delete.
        if serde_json::to_value(&config.files).unwrap()
            != serde_json::to_value(&self.config.files).unwrap()
        {
            return Err("请通过文件管理修改导入文件".into());
        }
        self.store.save(&config, overwrite)?;
        self.config = config;
        Ok(())
    }
    pub fn reload(&mut self) -> Result<()> {
        self.require_stopped()?;
        if !self.store.path().is_file() {
            return Err("config.json 不存在，保留当前有效配置".into());
        }
        let config = self.store.load()?;
        self.config = config;
        self.config_error = None;
        Ok(())
    }
    pub fn recover(&mut self, backup: bool) -> Result<()> {
        self.require_stopped()?;
        if self.config_error.is_none() {
            return Err("仅配置加载失败时可执行恢复".into());
        }
        self.config = self.store.recover(backup)?;
        self.config_error = None;
        Ok(())
    }
    pub fn import(&mut self, source: PathBuf, replace: Option<String>) -> Result<()> {
        if self.config_error.is_some() {
            return Err("请先恢复有效配置".into());
        }
        if !fs::metadata(&source).map_err(|e| e.to_string())?.is_file() {
            return Err("仅支持导入普通文件".into());
        }
        let mut config = self.config.clone();
        let file = if let Some(file_id) = replace {
            let referenced = |task: &Task| matches!(&task.command, Command::Composed { parts } if parts.iter().any(|p| matches!(p, Part::File { file_id: id } if id == &file_id)));
            if self.runs.iter().any(|r| {
                active(&r.status)
                    && (referenced(&r.snapshot)
                        || self
                            .config
                            .tasks
                            .iter()
                            .any(|t| t.id == r.task_id && referenced(t)))
            }) {
                return Err("引用该文件的任务正在运行，无法替换".into());
            }
            config
                .files
                .iter()
                .find(|f| f.id == file_id)
                .cloned()
                .ok_or("导入文件不存在")?
        } else {
            let name = source
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("文件名必须为有效的 Unicode")?
                .to_string();
            let file = ImportedFile {
                id: id(),
                name: name.clone(),
                stored_name: name,
            };
            config.files.push(file.clone());
            file
        };
        config.validate(&self.store.root)?;
        // Detect external edits even when replacing content with stable file identity.
        if fs::read(self.store.path()).ok() != self.store.disk {
            return Err("CONFIG_CONFLICT：请先处理外部配置修改".into());
        }
        let dir = self.store.root.join("files").join(&file.id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dest = dir.join(&file.stored_name);
        let staged = dir.join(format!("{}.import", id()));
        atomic_copy(&source, &staged)?;
        // Commit config before publishing a new import; roll back on failure.
        if let Err(e) = self.store.save(&config, false) {
            let _ = fs::remove_file(staged);
            return Err(e);
        }
        let result = atomic_copy(&staged, &dest);
        let _ = fs::remove_file(staged);
        if let Err(e) = result {
            let old = self.config.clone();
            self.store.save(&old, false)?;
            return Err(e);
        }
        self.config = config;
        Ok(())
    }
    pub fn delete_file(&mut self, file_id: &str) -> Result<()> {
        let referenced = |task: &Task| matches!(&task.command, Command::Composed { parts } if parts.iter().any(|p| matches!(p, Part::File { file_id: id } if id == file_id)));
        if self.config.tasks.iter().any(referenced)
            || self
                .runs
                .iter()
                .any(|r| active(&r.status) && referenced(&r.snapshot))
        {
            return Err("请先解除所有任务对该文件的引用，并停止引用它的运行实例".into());
        }
        if !self.config.files.iter().any(|f| f.id == file_id) {
            return Err("文件不存在".into());
        }
        let mut config = self.config.clone();
        config.files.retain(|f| f.id != file_id);
        self.store.save(&config, false)?;
        self.config = config;
        fs::remove_dir_all(self.store.root.join("files").join(file_id))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn log_path(&self, run_id: &str) -> PathBuf {
        self.store.root.join("logs").join(format!("{run_id}.jsonl"))
    }
    pub fn output(&self, run_id: &str) -> Result<String> {
        if !self.runs.iter().any(|r| r.id == run_id) {
            return Err("运行记录不存在".into());
        }
        match fs::read(self.log_path(run_id)) {
            Ok(s) => Ok(String::from_utf8_lossy(&s).into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn clear_output(&mut self, run_id: &str) -> Result<()> {
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or("运行记录不存在")?;
        run.truncated = false;
        atomic_write(&self.log_path(run_id), b"")?;
        self.persist()
    }
    pub fn clear_history(&mut self, task_id: &str) -> Result<()> {
        if self
            .runs
            .iter()
            .any(|r| r.task_id == task_id && active(&r.status))
        {
            return Err("请先停止任务再清除全部历史".into());
        }
        let ids: Vec<_> = self
            .runs
            .iter()
            .filter(|r| r.task_id == task_id)
            .map(|r| r.id.clone())
            .collect();
        self.runs.retain(|r| r.task_id != task_id);
        self.persist()?;
        for id in ids {
            let _ = fs::remove_file(self.log_path(&id));
        }
        Ok(())
    }
    pub fn cleanup(&mut self, run_id: &str, terminate: bool) -> Result<()> {
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or("运行记录不存在")?;
        if run.status != "abnormal" || self.controls.contains_key(&run.task_id) {
            return Err("此运行实例由应用管理，请使用停止".into());
        }
        if let Some(pid) = run.pid {
            if terminate {
                process::cleanup_orphan(pid, &run.id)?;
                thread::sleep(Duration::from_millis(100));
            }
            if process::orphan_alive(pid, &run.id)? {
                return Err("进程仍存活，暂不能再次启动此任务".into());
            }
        }
        run.status = "interrupted".into();
        run.ended_at = Some(now());
        run.message = Some("遗留运行实例已确认结束".into());
        self.persist()
    }
}

impl Shared {
    pub fn start(&self, task_id: &str) -> Result<()> {
        let mut hub = self.0.lock().map_err(|e| e.to_string())?;
        if hub.shutting_down {
            return Err("软件正在退出".into());
        }
        if let Some(e) = hub.config_error.as_ref().or(hub.runtime_error.as_ref()) {
            return Err(e.clone());
        }
        if hub
            .runs
            .iter()
            .any(|r| r.task_id == task_id && active(&r.status))
        {
            return Err("该任务已有运行实例，确认结束后才能再次启动".into());
        }
        let task = hub
            .config
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .cloned()
            .ok_or("任务不存在")?;
        let attempt = render(&task, &hub.config, &hub.store.root, true);
        let preview = render(&task, &hub.config, &hub.store.root, false)?;
        let run_id = id();
        let mut run = Run {
            id: run_id.clone(),
            task_id: task.id.clone(),
            task_name: task.name.clone(),
            snapshot: task.clone(),
            preview: PreviewRecord::from(&preview),
            status: "starting".into(),
            started_at: now(),
            ended_at: None,
            exit_code: None,
            pid: None,
            message: None,
            truncated: false,
        };
        // Journal the attempt before a process can exist. Recovery can locate a
        // process by its unique run marker even if the app dies before saving PID.
        hub.runs.insert(0, run.clone());
        prune(&mut hub, task_id);
        hub.persist()?;
        let result = attempt.and_then(|p| Process::spawn(&p, &run_id));
        let mut process = match result {
            Ok(p) => p,
            Err(e) => {
                run.status = "failed".into();
                run.ended_at = Some(now());
                run.message = Some(e.clone());
                hub.runs[0] = run;
                hub.persist()?;
                return Err(e);
            }
        };
        run.pid = Some(process.child.id());
        run.status = "running".into();
        hub.runs[0] = run;
        if let Err(e) = hub.persist() {
            let _ = process.force();
            let _ = process.child.wait();
            if let Some(r) = hub.runs.iter_mut().find(|r| r.id == run_id) {
                r.status = "failed".into();
                r.ended_at = Some(now());
                r.message = Some(e.clone());
            }
            return Err(e);
        }
        let control = Arc::new(Control {
            stop: AtomicBool::new(false),
        });
        hub.controls.insert(task.id.clone(), control.clone());
        let stdout = process.child.stdout.take().unwrap();
        let stderr = process.child.stderr.take().unwrap();
        drop(hub);
        let mut readers = vec![];
        for (stream, reader) in [
            ("stdout", Box::new(stdout) as Box<dyn Read + Send>),
            ("stderr", Box::new(stderr) as Box<dyn Read + Send>),
        ] {
            let shared = self.clone();
            let id = run_id.clone();
            readers.push(thread::spawn(move || {
                read_output(shared, id, stream, reader)
            }));
        }
        let shared = self.clone();
        thread::spawn(move || {
            let mut stopping = None;
            let mut forced = false;
            let mut exit = None;
            let mut error = None;
            loop {
                if control.stop.load(Ordering::SeqCst) && stopping.is_none() {
                    stopping = Some(Instant::now());
                    if let Err(e) = process.graceful() {
                        error = Some(e);
                    }
                }
                match process.child.try_wait() {
                    Ok(Some(status)) => {
                        exit = Some(status);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                }
                match process.tree_alive() {
                    Ok(false) if exit.is_some() => break,
                    Err(e) => {
                        error = Some(e);
                        break;
                    }
                    _ => {}
                }
                if stopping.is_some_and(|s| s.elapsed() >= Duration::from_secs(5)) && !forced {
                    if let Err(e) = process.force() {
                        error = Some(e);
                    }
                    forced = true;
                }
                if stopping.is_some_and(|s| s.elapsed() >= Duration::from_secs(8)) {
                    error = Some("无法确认进程树已结束，已保留异常状态并阻止重复启动".into());
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let ended = matches!(process.tree_alive(), Ok(false));
            if ended {
                for reader in readers {
                    let _ = reader.join();
                }
            }
            let mut hub = shared.0.lock().unwrap();
            if let Some(run) = hub.runs.iter_mut().find(|r| r.id == run_id) {
                run.ended_at = if ended { Some(now()) } else { None };
                run.exit_code = exit.and_then(|s| s.code());
                run.status = if !ended {
                    "abnormal"
                } else if stopping.is_some() {
                    "stopped"
                } else if exit.is_some_and(|s| s.success()) {
                    "succeeded"
                } else {
                    "failed"
                }
                .into();
                run.message = error;
            }
            hub.controls.remove(&task.id);
            let _ = hub.persist();
        });
        Ok(())
    }
    pub fn stop(&self, task_id: &str) -> Result<()> {
        let mut hub = self.0.lock().map_err(|e| e.to_string())?;
        let control = hub
            .controls
            .get(task_id)
            .ok_or("没有可停止的受管运行实例；异常实例请执行核查")?;
        control.stop.store(true, Ordering::SeqCst);
        if let Some(run) = hub
            .runs
            .iter_mut()
            .find(|r| r.task_id == task_id && active(&r.status))
        {
            run.status = "stopping".into();
        }
        hub.persist()
    }
    pub fn restart(&self, task_id: &str) -> Result<()> {
        let is_active = self
            .0
            .lock()
            .unwrap()
            .runs
            .iter()
            .any(|r| r.task_id == task_id && active(&r.status));
        if is_active {
            self.stop(task_id)?;
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                {
                    let hub = self.0.lock().unwrap();
                    if !hub.controls.contains_key(task_id) {
                        if hub
                            .runs
                            .iter()
                            .any(|r| r.task_id == task_id && active(&r.status))
                        {
                            return Err("旧运行实例未确认结束，取消重启".into());
                        }
                        break;
                    }
                }
                if Instant::now() > deadline {
                    return Err("等待旧运行实例结束超时".into());
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
        self.start(task_id)
    }
    pub fn shutdown(&self) -> Result<()> {
        {
            let mut hub = self.0.lock().unwrap();
            // No run can start in this session when the journal failed to load.
            // Leave the original file intact so the next launch stays blocked.
            if !hub.runtime_loaded {
                return Ok(());
            }
            hub.shutting_down = true;
            for control in hub.controls.values() {
                control.stop.store(true, Ordering::SeqCst);
            }
            for r in &mut hub.runs {
                if r.status == "running" {
                    r.status = "stopping".into();
                }
            }
            hub.persist()?;
        }
        let deadline = Instant::now() + Duration::from_secs(11);
        loop {
            let mut hub = self.0.lock().unwrap();
            if hub.controls.is_empty() {
                if hub.runs.iter().any(|r| active(&r.status)) {
                    hub.shutting_down = false;
                    return Err("仍有未确认结束的运行实例，请处理异常状态后退出".into());
                }
                return Ok(());
            }
            if Instant::now() > deadline {
                hub.shutting_down = false;
                return Err("停止任务超时，取消退出".into());
            }
            drop(hub);
            thread::sleep(Duration::from_millis(50));
        }
    }
}

fn valid_runs(runs: &[Run]) -> bool {
    let mut ids = std::collections::HashSet::new();
    runs.iter().all(|r| {
        valid_id(&r.id)
            && valid_id(&r.task_id)
            && r.snapshot.id == r.task_id
            && ids.insert(&r.id)
            && matches!(
                r.status.as_str(),
                "starting"
                    | "running"
                    | "stopping"
                    | "abnormal"
                    | "succeeded"
                    | "failed"
                    | "stopped"
                    | "interrupted"
            )
    })
}

fn prune(hub: &mut Hub, task_id: &str) {
    let mut count = 0;
    let mut removed = vec![];
    hub.runs.retain(|r| {
        if r.task_id != task_id {
            return true;
        }
        count += 1;
        if count <= 20 || active(&r.status) {
            true
        } else {
            removed.push(r.id.clone());
            false
        }
    });
    for id in removed {
        let _ = fs::remove_file(hub.log_path(&id));
    }
}

fn read_output(shared: Shared, run_id: String, stream: &str, mut reader: Box<dyn Read + Send>) {
    let mut buf = [0; 8192];
    let mut pending = Vec::new();
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                if !pending.is_empty() {
                    append_output(&shared, &run_id, stream, &String::from_utf8_lossy(&pending));
                }
                break;
            }
            Ok(n) => {
                pending.extend_from_slice(&buf[..n]);
                // Preserve UTF-8 sequences split across pipe reads, replace invalid bytes.
                let valid_len = match std::str::from_utf8(&pending) {
                    Ok(_) => pending.len(),
                    Err(e) if e.error_len().is_none() => e.valid_up_to(),
                    Err(_) => pending.len(),
                };
                if valid_len > 0 {
                    append_output(
                        &shared,
                        &run_id,
                        stream,
                        &String::from_utf8_lossy(&pending[..valid_len]),
                    );
                    pending.drain(..valid_len);
                }
            }
            Err(e) => {
                append_output(
                    &shared,
                    &run_id,
                    "stderr",
                    &format!("\n读取输出失败：{e}\n"),
                );
                break;
            }
        }
    }
}
fn append_output(shared: &Shared, run_id: &str, stream: &str, text: &str) {
    let mut hub = shared.0.lock().unwrap();
    let path = hub.log_path(run_id);
    let line = format!(
        "{}\n",
        serde_json::json!({"stream": stream, "text": text, "at": now()})
    );
    let mut first_truncation = false;
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        file.write_all(line.as_bytes())?;
        if file.metadata()?.len() > MAX_LOG as u64 {
            drop(file);
            let data = fs::read(&path)?;
            // Compact with headroom so sustained output does not rewrite 10 MB
            // for every small pipe read. The file always remains below the cap.
            let start = data.len().saturating_sub(MAX_LOG * 9 / 10);
            let start = data[start..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|p| start + p + 1)
                .unwrap_or(data.len());
            atomic_write(&path, &data[start..]).map_err(std::io::Error::other)?;
            if let Some(run) = hub.runs.iter_mut().find(|r| r.id == run_id) {
                first_truncation = !run.truncated;
                run.truncated = true;
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        hub.runtime_error = Some(format!("输出日志保存失败：{e}"));
    }
    if first_truncation {
        let _ = hub.persist();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Fixture {
        shared: Shared,
        root: PathBuf,
    }
    impl Fixture {
        fn new(command: &str) -> Self {
            let root = std::env::temp_dir().join(format!("script-hub-runtime-{}", id()));
            let mut hub = Hub::open(root.clone()).unwrap();
            hub.config.tasks.push(Task {
                id: "task".into(),
                name: "测试任务".into(),
                command: Command::Direct {
                    text: command.into(),
                },
                working_directory: root.to_string_lossy().into(),
                environment: BTreeMap::new(),
                variables: BTreeMap::new(),
                auto_start: false,
            });
            let config = hub.config.clone();
            hub.store.save(&config, false).unwrap();
            Self {
                shared: Shared(Arc::new(Mutex::new(hub))),
                root,
            }
        }
        fn wait(&self) -> Run {
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                {
                    let hub = self.shared.0.lock().unwrap();
                    if hub.controls.is_empty() {
                        return hub.runs[0].clone();
                    }
                }
                assert!(Instant::now() < deadline, "运行未在期限内结束");
                thread::sleep(Duration::from_millis(30));
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.shared.shutdown();
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn missing_variable_records_failure_without_spawning() {
        let f = Fixture::new("echo ${required}");
        assert!(f.shared.start("task").unwrap_err().contains("required"));
        let run = f.wait();
        assert_eq!(run.status, "failed");
        assert!(run.pid.is_none());
    }

    #[test]
    fn failed_reload_keeps_memory_and_reload_never_starts_auto_tasks() {
        let f = Fixture::new("echo test");
        let mut hub = f.shared.0.lock().unwrap();
        fs::write(hub.store.path(), b"{broken").unwrap();
        assert!(hub.reload().is_err());
        assert_eq!(hub.config.tasks[0].name, "测试任务");
        let mut disk = hub.config.clone();
        disk.tasks[0].auto_start = true;
        fs::write(hub.store.path(), serde_json::to_vec(&disk).unwrap()).unwrap();
        hub.reload().unwrap();
        assert!(hub.config.tasks[0].auto_start);
        assert!(hub.controls.is_empty() && hub.runs.is_empty());
    }

    #[test]
    fn damaged_runtime_journal_is_never_replaced_with_empty_history() {
        let f = Fixture::new("echo test");
        let path = f.root.join("runs.json");
        fs::write(&path, b"broken journal").unwrap();
        let mut hub = Hub::open(f.root.clone()).unwrap();
        assert!(hub.runtime_error.is_some());
        assert!(hub.clear_history("task").is_err());
        let shared = Shared(Arc::new(Mutex::new(hub)));
        assert!(shared.start("task").is_err());
        shared.shutdown().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"broken journal");
    }

    #[cfg(windows)]
    #[test]
    fn windows_full_command_retains_redirection_and_exit_code() {
        let f = Fixture::new("echo output & echo error 1>&2 & exit /b 7");
        f.shared.start("task").unwrap();
        let run = f.wait();
        assert_eq!(run.status, "failed");
        assert_eq!(run.exit_code, Some(7));
        let output = f.shared.0.lock().unwrap().output(&run.id).unwrap();
        assert!(output.contains("output") && output.contains("error") && output.contains("stderr"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_job_owns_children_and_blocks_duplicate_start() {
        let f = Fixture::new("ping -t 127.0.0.1 >NUL");
        f.shared.start("task").unwrap();
        assert!(f.shared.start("task").is_err());
        f.shared.stop("task").unwrap();
        let run = f.wait();
        assert_eq!(run.status, "stopped");
        assert!(!process::orphan_alive(run.pid.unwrap(), &run.id).unwrap());
    }

    #[test]
    fn file_references_prevent_deletion_and_replacement_keeps_path() {
        let f = Fixture::new("echo test");
        let source = f.root.join("data.txt");
        fs::write(&source, "first").unwrap();
        let mut hub = f.shared.0.lock().unwrap();
        hub.import(source.clone(), None).unwrap();
        hub.import(source.clone(), None).unwrap();
        let file = hub.config.files[0].clone();
        assert_ne!(file.id, hub.config.files[1].id);
        fs::write(&source, "replacement").unwrap();
        hub.import(source, Some(file.id.clone())).unwrap();
        assert_eq!(
            fs::read_to_string(
                hub.store
                    .root
                    .join("files")
                    .join(&file.id)
                    .join(&file.stored_name)
            )
            .unwrap(),
            "replacement"
        );
        hub.config.tasks[0].command = Command::Composed {
            parts: vec![
                Part::Program {
                    value: "echo".into(),
                },
                Part::File {
                    file_id: file.id.clone(),
                },
            ],
        };
        assert!(hub.delete_file(&file.id).unwrap_err().contains("引用"));
    }

    #[cfg(unix)]
    #[test]
    fn full_command_stdout_stderr_environment_and_history_survive_reopen() {
        let f = Fixture::new("printf '%s' \"$VALUE\"; printf 'error' >&2; exit 7");
        {
            let mut hub = f.shared.0.lock().unwrap();
            hub.config.tasks[0]
                .environment
                .insert("VALUE".into(), "中文 ${name}".into());
            hub.config.tasks[0].variables.insert(
                "name".into(),
                Variable {
                    value: Some("ok".into()),
                    allow_empty: false,
                },
            );
        }
        f.shared.start("task").unwrap();
        let run = f.wait();
        assert_eq!(run.status, "failed");
        assert_eq!(run.exit_code, Some(7));
        let hub = f.shared.0.lock().unwrap();
        let output = hub.output(&run.id).unwrap();
        assert!(
            output.contains("中文 ok") && output.contains("stderr") && output.contains("error")
        );
        drop(hub);
        let reopened = Hub::open(f.root.clone()).unwrap();
        assert_eq!(reopened.runs[0].exit_code, Some(7));
        assert_eq!(reopened.output(&run.id).unwrap(), output);
    }

    #[cfg(unix)]
    #[test]
    fn duplicate_start_blocked_snapshot_immutable_and_entire_tree_stopped() {
        let f = Fixture::new("sleep 60 & wait");
        f.shared.start("task").unwrap();
        assert!(f.shared.start("task").is_err());
        {
            let mut hub = f.shared.0.lock().unwrap();
            let mut edited = hub.config.clone();
            edited.tasks[0].name = "修改后的任务".into();
            hub.save(edited, false).unwrap();
            assert_eq!(hub.runs[0].snapshot.name, "测试任务");
            assert!(hub.reload().is_err());
            let mut deleted = hub.config.clone();
            deleted.tasks.clear();
            assert!(hub.save(deleted, false).is_err());
        }
        f.shared.stop("task").unwrap();
        let run = f.wait();
        assert_eq!(run.status, "stopped");
        assert!(!process::orphan_alive(run.pid.unwrap(), &run.id).unwrap());
        f.shared.start("task").unwrap();
        f.shared.stop("task").unwrap();
        assert_eq!(f.wait().snapshot.name, "修改后的任务");
    }

    #[cfg(unix)]
    #[test]
    fn restart_waits_for_old_tree_and_uses_saved_command() {
        let f = Fixture::new("sleep 60");
        f.shared.start("task").unwrap();
        let old_id = f.shared.0.lock().unwrap().runs[0].id.clone();
        {
            let mut hub = f.shared.0.lock().unwrap();
            let mut config = hub.config.clone();
            config.tasks[0].command = Command::Direct {
                text: "printf restarted".into(),
            };
            hub.save(config, false).unwrap();
        }
        f.shared.restart("task").unwrap();
        let run = f.wait();
        assert_eq!(run.status, "succeeded");
        let hub = f.shared.0.lock().unwrap();
        assert_eq!(
            hub.runs.iter().find(|r| r.id == old_id).unwrap().status,
            "stopped"
        );
        assert!(hub.output(&run.id).unwrap().contains("restarted"));
    }

    #[cfg(unix)]
    #[test]
    fn uncooperative_process_tree_is_forced_after_grace_period() {
        let f = Fixture::new("trap '' TERM; printf ready; sleep 60 & wait");
        f.shared.start("task").unwrap();
        // Wait for the shell's explicit readiness marker rather than racing its trap.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let ready = {
                let hub = f.shared.0.lock().unwrap();
                hub.output(&hub.runs[0].id).unwrap().contains("ready")
            };
            if ready {
                break;
            }
            assert!(Instant::now() < deadline, "未收到就绪输出");
            thread::sleep(Duration::from_millis(10));
        }
        let before = Instant::now();
        f.shared.stop("task").unwrap();
        assert_eq!(f.wait().status, "stopped");
        assert!(before.elapsed() >= Duration::from_secs(5));
    }

    #[test]
    fn output_is_bounded_clearable_and_history_is_pruned() {
        let f = Fixture::new("echo test");
        {
            let mut hub = f.shared.0.lock().unwrap();
            for index in 0..25 {
                let task = hub.config.tasks[0].clone();
                hub.runs.push(Run {
                    id: format!("run-{index}"),
                    task_id: task.id.clone(),
                    task_name: task.name.clone(),
                    preview: PreviewRecord {
                        command: String::new(),
                        working_directory: String::new(),
                        environment: BTreeMap::new(),
                    },
                    snapshot: task,
                    status: "succeeded".into(),
                    started_at: now(),
                    ended_at: Some(now()),
                    exit_code: Some(0),
                    pid: None,
                    message: None,
                    truncated: false,
                });
            }
            prune(&mut hub, "task");
            assert_eq!(hub.runs.len(), 20);
        }
        let text = "x".repeat(8192);
        for _ in 0..1300 {
            append_output(&f.shared, "run-0", "stdout", &text);
        }
        let mut hub = f.shared.0.lock().unwrap();
        assert!(fs::metadata(hub.log_path("run-0")).unwrap().len() <= MAX_LOG as u64);
        assert!(hub.runs[0].truncated);
        for line in hub.output("run-0").unwrap().lines() {
            assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
        }
        hub.clear_output("run-0").unwrap();
        assert!(hub.output("run-0").unwrap().is_empty());
        hub.clear_history("task").unwrap();
        assert!(hub.runs.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn recovery_finds_process_when_crash_precedes_pid_journal() {
        let f = Fixture::new("sleep 60");
        f.shared.start("task").unwrap();
        {
            let mut hub = f.shared.0.lock().unwrap();
            hub.runs[0].pid = None;
            hub.persist().unwrap();
        }
        let mut recovered = Hub::open(f.root.clone()).unwrap();
        assert_eq!(recovered.runs[0].status, "abnormal");
        assert!(recovered.runs[0].pid.is_some());
        let id = recovered.runs[0].id.clone();
        recovered.cleanup(&id, true).unwrap();
        assert_eq!(recovered.runs[0].status, "interrupted");
        f.wait();
    }
}
