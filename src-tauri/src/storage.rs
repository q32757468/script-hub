use crate::model::Config;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        atomic_replace(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub fn atomic_copy(source: &Path, dest: &Path) -> Result<(), String> {
    let temp = dest.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        // Preserve Unix executable bits. Managed Windows copies remain writable
        // even when the source has the read-only attribute, so replacement works.
        #[cfg(unix)]
        output
            .set_permissions(input.metadata().map_err(|e| e.to_string())?.permissions())
            .map_err(|e| e.to_string())?;
        drop(output);
        atomic_replace(&temp, dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn atomic_replace(temp: &Path, path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
        let old: Vec<_> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let new: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(old.as_ptr(), new.as_ptr(), 0x1 | 0x8) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::rename(temp, path).map_err(|e| e.to_string())
    }
}

pub struct Store {
    pub root: PathBuf,
    pub disk: Option<Vec<u8>>,
}

impl Store {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        for dir in ["files", "workspaces", "logs"] {
            fs::create_dir_all(root.join(dir)).map_err(|e| e.to_string())?;
        }
        Ok(Self { root, disk: None })
    }
    pub fn path(&self) -> PathBuf {
        self.root.join("config.json")
    }
    pub fn parse(&self, bytes: &[u8]) -> Result<Config, String> {
        // Check version before decoding fields so newer schemas have a clear failure.
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| format!("配置 JSON 无效：{e}"))?;
        if value.get("version").and_then(|v| v.as_u64()) != Some(crate::model::VERSION as u64) {
            return Err("不支持此配置版本，请检查 version 或升级软件".into());
        }
        let config: Config =
            serde_json::from_value(value).map_err(|e| format!("配置结构无效：{e}"))?;
        config.validate(&self.root)?;
        Ok(config)
    }
    pub fn load(&mut self) -> Result<Config, String> {
        match fs::read(self.path()) {
            Ok(bytes) => {
                let config = self.parse(&bytes)?;
                self.disk = Some(bytes);
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Config::default();
                self.save(&config, false)?;
                Ok(config)
            }
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&mut self, config: &Config, overwrite: bool) -> Result<(), String> {
        config.validate(&self.root)?;
        let disk = match fs::read(self.path()) {
            Ok(v) => Some(v),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if !overwrite && disk != self.disk {
            return Err(
                "CONFIG_CONFLICT：磁盘配置已被外部修改。请选择加载磁盘版本或覆盖磁盘版本。".into(),
            );
        }
        if let Some(bytes) = &disk {
            if self.parse(bytes).is_ok() {
                atomic_write(&self.root.join("config.backup.json"), bytes)?;
            } else if overwrite {
                // Preserve the invalid original while retaining the last valid backup.
                atomic_write(
                    &self
                        .root
                        .join(format!("config.damaged-{}.json", uuid::Uuid::new_v4())),
                    bytes,
                )?;
            } else {
                return Err("磁盘配置损坏，未写入；请先选择恢复方式".into());
            }
        }
        let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
        // Drafts only display the default path. Materialize managed directories
        // when committing valid configuration, after checking disk conflicts.
        let mut created = Vec::new();
        let result = (|| {
            for task in &config.tasks {
                let path = self.root.join("workspaces").join(&task.id);
                if Path::new(&task.working_directory) != path {
                    continue;
                }
                match fs::create_dir(&path) {
                    Ok(()) => created.push(path),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
                    Err(e) => return Err(format!("无法创建任务工作目录：{e}")),
                }
            }
            atomic_write(&self.path(), &bytes)
        })();
        if result.is_err() {
            for path in created.iter().rev() {
                // Remove only empty directories created by this save.
                let _ = fs::remove_dir(path);
            }
        }
        result?;
        self.disk = Some(bytes);
        Ok(())
    }
    pub fn recover(&mut self, backup: bool) -> Result<Config, String> {
        let config = if backup {
            self.parse(&fs::read(self.root.join("config.backup.json")).map_err(|e| e.to_string())?)?
        } else {
            Config::default()
        };
        self.save(&config, true)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Command, Task};

    fn task(root: &Path, id: &str) -> Task {
        Task {
            id: id.into(),
            name: "测试任务".into(),
            command: Command::Direct {
                text: "echo test".into(),
            },
            working_directory: root.join("workspaces").join(id).to_string_lossy().into(),
            environment: Default::default(),
            variables: Default::default(),
            auto_start: false,
        }
    }

    #[test]
    fn workspace_is_created_on_save_only_and_custom_directory_is_preserved() {
        let root = std::env::temp_dir().join(format!("script-hub-test-{}", uuid::Uuid::new_v4()));
        let mut store = Store::new(root.clone()).unwrap();
        let mut config = store.load().unwrap();
        let draft = task(&root, "new-task");
        let path = PathBuf::from(&draft.working_directory);
        assert!(!path.exists());
        config.tasks.push(draft);
        store.save(&config, false).unwrap();
        assert!(path.is_dir());
        assert_eq!(store.load().unwrap().tasks[0].id, "new-task");

        let mut custom = task(&root, "custom-task");
        custom.working_directory = root.to_string_lossy().into();
        config.tasks.push(custom);
        fs::write(path.join("keep.txt"), "user data").unwrap();
        store.save(&config, false).unwrap();
        assert!(!root.join("workspaces/custom-task").exists());
        assert_eq!(fs::read(path.join("keep.txt")).unwrap(), b"user data");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_or_conflicting_save_does_not_create_workspace() {
        let root = std::env::temp_dir().join(format!("script-hub-test-{}", uuid::Uuid::new_v4()));
        let mut store = Store::new(root.clone()).unwrap();
        let mut config = store.load().unwrap();
        config.tasks.push(task(&root, "new-task"));
        config.tasks[0].name.clear();
        assert!(store.save(&config, false).is_err());
        assert!(!root.join("workspaces/new-task").exists());

        config.tasks[0].name = "测试任务".into();
        fs::write(store.path(), b"external change").unwrap();
        assert!(
            store
                .save(&config, false)
                .unwrap_err()
                .contains("CONFIG_CONFLICT")
        );
        assert!(!root.join("workspaces/new-task").exists());
        store.save(&config, true).unwrap();
        assert!(root.join("workspaces/new-task").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_workspace_creation_rolls_back_only_new_empty_directories() {
        let root = std::env::temp_dir().join(format!("script-hub-test-{}", uuid::Uuid::new_v4()));
        let mut store = Store::new(root.clone()).unwrap();
        let mut config = store.load().unwrap();
        let original = fs::read(store.path()).unwrap();
        let previous_disk = store.disk.clone();
        let existing = root.join("workspaces/existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("keep.txt"), "user data").unwrap();
        let blocked = root.join("workspaces/blocked");
        fs::write(&blocked, "existing file").unwrap();
        for id in ["existing", "new-task", "blocked"] {
            config.tasks.push(task(&root, id));
        }
        assert!(store.save(&config, false).is_err());
        assert!(!root.join("workspaces/new-task").exists());
        assert_eq!(fs::read(existing.join("keep.txt")).unwrap(), b"user data");
        assert_eq!(fs::read(blocked).unwrap(), b"existing file");
        assert_eq!(fs::read(store.path()).unwrap(), original);
        assert_eq!(store.disk, previous_disk);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn conflict_backup_and_failed_reload_preserve_data() {
        let root = std::env::temp_dir().join(format!("script-hub-test-{}", uuid::Uuid::new_v4()));
        let mut store = Store::new(root.clone()).unwrap();
        let config = store.load().unwrap();
        let original = fs::read(store.path()).unwrap();
        fs::write(store.path(), b"broken").unwrap();
        assert!(
            store
                .save(&config, false)
                .unwrap_err()
                .contains("CONFIG_CONFLICT")
        );
        assert!(store.load().is_err());
        assert_eq!(store.disk, Some(original));
        store.save(&config, true).unwrap();
        let mut next = config.clone();
        next.settings.launch_at_login = true;
        store.save(&next, false).unwrap();
        assert!(
            !store
                .parse(&fs::read(root.join("config.backup.json")).unwrap())
                .unwrap()
                .settings
                .launch_at_login
        );
        assert!(!store.recover(true).unwrap().settings.launch_at_login);
        fs::remove_dir_all(root).unwrap();
    }
}
