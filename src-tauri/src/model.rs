use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path};

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub launch_at_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Variable {
    pub value: Option<String>,
    #[serde(default)]
    pub allow_empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Part {
    Program { value: String },
    Text { value: String },
    File { file_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum Command {
    Direct { text: String },
    Composed { parts: Vec<Part> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub command: Command,
    pub working_directory: String,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub variables: BTreeMap<String, Variable>,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportedFile {
    pub id: String,
    pub name: String,
    pub stored_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    pub tasks: Vec<Task>,
    pub files: Vec<ImportedFile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: VERSION,
            settings: Settings::default(),
            tasks: vec![],
            files: vec![],
        }
    }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn variable_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

// A single left-to-right pass keeps escaped placeholders and substituted values literal.
pub fn substitute(
    text: &str,
    variables: &BTreeMap<String, Variable>,
    names: &mut HashSet<String>,
    strict: bool,
) -> Result<String, String> {
    let mut result = String::new();
    let mut i = 0;
    while i < text.len() {
        let escaped = text[i..].starts_with("$${");
        let prefix = if escaped { 3 } else { 2 };
        if (escaped || text[i..].starts_with("${"))
            && let Some(end) = text[i + prefix..].find('}')
        {
            let name = &text[i + prefix..i + prefix + end];
            if variable_name(name) {
                if escaped {
                    result.push_str(&format!("${{{name}}}"));
                } else {
                    names.insert(name.to_owned());
                    match variables.get(name) {
                        Some(v)
                            if v.allow_empty || v.value.as_ref().is_some_and(|s| !s.is_empty()) =>
                        {
                            result.push_str(v.value.as_deref().unwrap_or(""));
                        }
                        _ if strict => return Err(format!("输入变量「{name}」尚未赋值")),
                        _ => result.push_str(&format!("${{{name}}}")),
                    }
                }
                i += prefix + end + 1;
                continue;
            }
        }
        let c = text[i..].chars().next().unwrap();
        result.push(c);
        i += c.len_utf8();
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub command: String,
    pub working_directory: String,
    pub environment: BTreeMap<String, String>,
    pub variable_names: Vec<String>,
}

pub fn render(task: &Task, config: &Config, root: &Path, strict: bool) -> Result<Preview, String> {
    let mut names = HashSet::new();
    let mut sub = |s: &str| substitute(s, &task.variables, &mut names, strict);
    let command = match &task.command {
        Command::Direct { text } => sub(text)?,
        Command::Composed { parts } => parts
            .iter()
            .map(|part| match part {
                Part::Program { value } | Part::Text { value } => sub(value),
                Part::File { file_id } => {
                    let file = config
                        .files
                        .iter()
                        .find(|f| &f.id == file_id)
                        .ok_or_else(|| format!("导入文件不存在：{file_id}"))?;
                    let path = root.join("files").join(&file.id).join(&file.stored_name);
                    if strict && !path.is_file() {
                        return Err(format!("导入文件已丢失：{}", file.name));
                    }
                    quote_file(&path.to_string_lossy())
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(" "),
    };
    let working_directory = sub(&task.working_directory)?;
    let environment = task
        .environment
        .iter()
        .map(|(k, v)| Ok((k.clone(), sub(v)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut variable_names: Vec<_> = names.into_iter().collect();
    variable_names.sort();
    Ok(Preview {
        command,
        working_directory,
        environment,
        variable_names,
    })
}

fn quote_file(path: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        // cmd expands % even within quotes. Refuse an unrepresentable managed path.
        if path.contains(['%', '!', '"', '\r', '\n']) {
            return Err("当前文件路径含命令解释器无法安全引用的字符，请更改用户目录".into());
        }
        Ok(format!("\"{path}\""))
    }
    #[cfg(not(windows))]
    {
        Ok(format!("'{}'", path.replace('\'', "'\\''")))
    }
}

impl Config {
    pub fn validate(&self, root: &Path) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!(
                "不支持配置版本 {}，当前支持版本 {VERSION}。请使用匹配版本的软件。",
                self.version
            ));
        }
        let mut ids = HashSet::new();
        for file in &self.files {
            let valid_name = |s: &str| {
                !s.is_empty()
                    && !s.contains(['/', '\\', '\0'])
                    && Path::new(s).components().count() == 1
                    && matches!(Path::new(s).components().next(), Some(Component::Normal(_)))
            };
            if !valid_id(&file.id)
                || !ids.insert(&file.id)
                || !valid_name(&file.stored_name)
                || !valid_name(&file.name)
            {
                return Err(format!("导入文件身份、名称或存储路径无效：{}", file.name));
            }
        }
        ids.clear();
        for task in &self.tasks {
            if !valid_id(&task.id) || !ids.insert(&task.id) || task.name.trim().is_empty() {
                return Err("任务名称必填，任务 ID 必须有效且唯一".into());
            }
            if task.working_directory.trim().is_empty() || task.working_directory.contains('\0') {
                return Err(format!("任务「{}」的工作目录无效", task.name));
            }
            if let Command::Composed { parts } = &task.command {
                if !matches!(parts.first(), Some(Part::Program { value }) if !value.trim().is_empty())
                    || parts
                        .iter()
                        .skip(1)
                        .any(|p| matches!(p, Part::Program { .. }))
                {
                    return Err("组合命令首项必须是程序，后续仅允许文件引用与文本".into());
                }
            } else if let Command::Direct { text } = &task.command
                && text.trim().is_empty()
            {
                return Err(format!("任务「{}」的命令不能为空", task.name));
            }
            if task
                .environment
                .iter()
                .any(|(k, v)| k.is_empty() || k.contains(['=', '\0']) || v.contains('\0'))
            {
                return Err("任务环境变量的名称或值无效".into());
            }
            if task.variables.keys().any(|key| !variable_name(key)) {
                return Err("输入变量名称无效".into());
            }
            let p = render(task, self, root, false)?;
            if p.command.contains('\0')
                || p.working_directory.contains('\0')
                || p.environment.values().any(|v| v.contains('\0'))
            {
                return Err("替换后的命令、工作目录和环境变量不能含空字符".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placeholders_are_case_sensitive_escaped_and_non_recursive() {
        let vars = BTreeMap::from([
            (
                "name".into(),
                Variable {
                    value: Some("${other}".into()),
                    allow_empty: false,
                },
            ),
            (
                "empty".into(),
                Variable {
                    value: None,
                    allow_empty: true,
                },
            ),
        ]);
        let mut names = HashSet::new();
        assert_eq!(
            substitute(
                "$${skip} ${name} ${empty} ${2bad} 中文",
                &vars,
                &mut names,
                true
            )
            .unwrap(),
            "${skip} ${other}  ${2bad} 中文"
        );
        assert_eq!(
            names,
            HashSet::from(["name".to_string(), "empty".to_string()])
        );
        assert!(substitute("${Name}", &vars, &mut names, true).is_err());
    }
}
