import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { call, desktop, emptySnapshot } from "./api";
import {
  active,
  statusText,
  parseOutput,
  variableNames,
  normalizeTask,
  duration,
  dateTime,
  type Config,
  type Task,
  type Part,
  type Preview,
  type Output,
  type Snapshot,
} from "./domain";
import { Badge, Button, Card, Icon, MetricCard, Modal } from "./components/ui";
import { TitleBar } from "./components/TitleBar";

type Page = "tasks" | "files" | "settings";
type Confirmation = {
  title: string;
  body: string;
  action: () => void;
  label?: string;
};
const clone = <T,>(value: T): T => structuredClone(value);
function App() {
  const [data, setData] = useState<Snapshot>({
    ...emptySnapshot,
    root: desktop ? "" : emptySnapshot.root,
  });
  const [page, setPage] = useState<Page>("tasks");
  const isSettingsPage = page === "settings";
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState<Task | null>(null);
  const [baseline, setBaseline] = useState("");
  const [taskDialog, setTaskDialog] = useState<"edit" | "logs" | null>(null);
  const [query, setQuery] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const searchToggleRef = useRef<HTMLButtonElement>(null);
  const [working, setWorking] = useState(false);
  const busy = working && !isSettingsPage;
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [pendingConfig, setPendingConfig] = useState<Config | null>(null);
  const [confirm, setConfirm] = useState<Confirmation | null>(null);
  const [previewResult, setPreview] = useState<Preview | null>(null);
  const [previewFor, setPreviewFor] = useState("");
  const [previewError, setPreviewError] = useState("");
  const [chosenRun, setChosenRun] = useState("");
  const [output, setOutput] = useState<Output[]>([]);
  const [filter, setFilter] = useState("all");
  const [follow, setFollow] = useState(true);
  const [clock, setClock] = useState(Date.now());
  const outputRef = useRef<HTMLDivElement>(null);
  const preview = previewFor === JSON.stringify(draft) ? previewResult : null;
  const dirty = !!draft && JSON.stringify(draft) !== baseline;
  const taskRuns = data.runs.filter((r) => r.taskId === selected);
  const latest = taskRuns[0];
  const run = taskRuns.find((r) => r.id === chosenRun) ?? latest;
  const running = taskRuns.find((r) => active(r.status));
  const names = draft ? variableNames(draft) : [];
  const missing = names.filter(
    (n) => !draft?.variables[n]?.allowEmpty && !draft?.variables[n]?.value,
  );
  const refresh = useCallback(async () => {
    if (!desktop) return emptySnapshot;
    const next = await call<Snapshot>("snapshot");
    setData((previous) => {
      const configIdentical =
        JSON.stringify(previous.config) === JSON.stringify(next.config);
      const runsIdentical =
        JSON.stringify(previous.runs) === JSON.stringify(next.runs);
      if (
        configIdentical &&
        runsIdentical &&
        previous.root === next.root &&
        previous.configError === next.configError &&
        previous.runtimeError === next.runtimeError
      ) {
        return previous;
      }
      return {
        ...next,
        config: configIdentical ? previous.config : next.config,
        runs: runsIdentical ? previous.runs : next.runs,
      };
    });
    return next;
  }, []);
  const loadTask = (task: Task | null) => {
    setSelected(task?.id ?? null);
    setDraft(task ? clone(task) : null);
    setBaseline(task ? JSON.stringify(task) : "");
    setChosenRun("");
    setOutput([]);
  };
  const work = async (action: () => Promise<void>) => {
    if (!isSettingsPage) setWorking(true);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (e) {
      if (String(e).includes("CONFIG_CONFLICT"))
        setPendingConfig((current) => current ?? clone(data.config));
      setError(String(e));
    } finally {
      if (!isSettingsPage) setWorking(false);
    }
  };
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        if (!disposed) await refresh();
      } catch (e) {
        if (!disposed) setError(String(e));
      }
      if (!disposed) timer = setTimeout(poll, 1000);
    }
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [refresh]);
  useEffect(() => {
    const timer = setInterval(() => setClock(Date.now()), 1000);
    if (!desktop) return () => clearInterval(timer);
    const unlisten = listen<string>("hub-error", (event) =>
      setError(event.payload),
    );
    return () => {
      clearInterval(timer);
      void unlisten.then((fn) => fn());
    };
  }, []);
  useEffect(() => {
    let disposed = false;
    setPreview(null);
    setPreviewError("");
    if (!draft || !desktop) return;
    const timer = setTimeout(() => {
      call<Preview>("preview_command", { task: normalizeTask(draft) })
        .then((p) => {
          if (!disposed) {
            setPreview(p);
            setPreviewFor(JSON.stringify(draft));
          }
        })
        .catch((e) => {
          if (!disposed) setPreviewError(String(e));
        });
    }, 180);
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [draft, data.config.files]);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    setOutput([]);
    if (taskDialog !== "logs" || !run || !desktop) return;
    async function poll() {
      try {
        const raw = await call<string>("read_output", { runId: run!.id });
        if (!disposed) setOutput(parseOutput(raw));
      } catch (e) {
        if (!disposed) setError(String(e));
      }
      if (!disposed) timer = setTimeout(poll, 800);
    }
    void poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [run?.id, taskDialog]);
  useEffect(() => {
    if (follow && outputRef.current)
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
  }, [output, follow]);
  function closeTaskDialog() {
    if (busy) return;
    guarded(() => setTaskDialog(null));
  }
  const closeConfirm = useCallback(() => {
    setConfirm(null);
  }, []);
  const closePendingConfig = useCallback(() => {
    setPendingConfig(null);
  }, []);
  function openTaskDialog(task: Task, dialog: "edit" | "logs") {
    setError("");
    setNotice("");
    loadTask(task);
    setTaskDialog(dialog);
  }
  function guarded(action: () => void) {
    if (dirty)
      setConfirm({
        title: "放弃未保存的修改？",
        body: "当前任务的修改尚未保存，继续后会丢失这些修改。",
        action,
        label: "放弃修改并继续",
      });
    else action();
  }
  async function persist(config: Config, overwrite = false) {
    try {
      await call("save_config", { config, overwrite });
    } catch (e) {
      if (String(e).includes("CONFIG_CONFLICT")) setPendingConfig(config);
      throw e;
    }
    await refresh();
  }
  async function saveTask() {
    if (!draft) return;
    const task = normalizeTask(draft);
    const config = clone(data.config);
    const index = config.tasks.findIndex((t) => t.id === task.id);
    if (index < 0) config.tasks.push(task);
    else config.tasks[index] = task;
    await persist(config);
    setDraft(task);
    setBaseline(JSON.stringify(task));
    setNotice(running ? "已保存，将在下次运行时生效。" : "任务已保存。");
  }
  function workspacePath(taskId: string) {
    const separator = data.root.includes("\\") ? "\\" : "/";
    return `${data.root}${separator}workspaces${separator}${taskId}`;
  }
  function newTask() {
    const id = crypto.randomUUID();
    const task: Task = {
      id,
      name: "",
      command: { mode: "direct", text: "" },
      workingDirectory: workspacePath(id),
      environment: {},
      variables: {},
      autoStart: false,
    };
    setError("");
    setNotice("");
    setPage("tasks");
    loadTask(task);
    setTaskDialog("edit");
  }
  async function duplicateTask() {
    if (!draft) return;
    const task = normalizeTask(clone(draft));
    task.id = crypto.randomUUID();
    task.name += "（副本）";
    task.autoStart = false;
    const oldDefault = workspacePath(draft.id);
    if (task.workingDirectory === oldDefault)
      task.workingDirectory = workspacePath(task.id);
    await persist({ ...data.config, tasks: [...data.config.tasks, task] });
    loadTask(task);
    setNotice("已创建独立任务副本，任务自动启动默认关闭。");
  }
  async function taskAction(action: string) {
    if (!draft) return;
    if (action !== "stop" && dirty) await saveTask();
    await call("run_task", { taskId: draft.id, action });
    setChosenRun("");
    await refresh();
  }
  async function chooseDirectory() {
    const path = await call<string | null>("choose_path", { directory: true });
    if (path && draft) setDraft({ ...draft, workingDirectory: path });
  }
  async function importFile(replace: string | null = null) {
    const source = await call<string | null>("choose_path", {
      directory: false,
    });
    if (source) {
      await call("import_file", { source, replace });
      await refresh();
      setNotice(
        replace ? "文件已替换，原存储路径与引用保持不变。" : "文件已导入。",
      );
    }
  }
  function updatePart(index: number, part: Part) {
    if (draft?.command.mode !== "composed") return;
    const parts = [...draft.command.parts];
    parts[index] = part;
    setDraft({ ...draft, command: { mode: "composed", parts } });
  }
  function movePart(index: number, direction: number) {
    if (draft?.command.mode !== "composed") return;
    const parts = [...draft.command.parts];
    [parts[index], parts[index + direction]] = [
      parts[index + direction],
      parts[index],
    ];
    setDraft({ ...draft, command: { mode: "composed", parts } });
  }
  const openDirectory = (fileId: string | null = null) =>
    work(async () => {
      await call("open_directory", { fileId });
    });
  const reload = () =>
    work(async () => {
      await call("reload_config");
      const next = await refresh();
      loadTask(next.config.tasks.find((t) => t.id === selected) ?? null);
      setPendingConfig(null);
      setNotice("已加载磁盘配置。任务自动启动不会再次触发。");
    });
  const totalActive = data.runs.filter((r) => active(r.status)).length;
  const canCreateTask = desktop && !!data.root && !data.configError && !busy;

  return (
    <div className="app-shell">
      <TitleBar />
      <div className="app-body">
        <aside
          className="sidebar"
          inert={!!taskDialog || !!confirm || !!pendingConfig}
        >
          <div className="brand" data-tauri-drag-region>
            <span className="brand-icon">
              <Icon name="terminal" size={24} />
            </span>
            <div>
              Script Hub<small>本机任务工作台</small>
            </div>
          </div>
          <div className="nav-caption">工作空间</div>
          <nav>
            {(["tasks", "files", "settings"] as Page[]).map((p) => (
              <button
                key={p}
                className={page === p ? "nav-item selected" : "nav-item"}
                onClick={() =>
                  guarded(() => {
                    setPage(p);
                    if (p !== "tasks") loadTask(null);
                  })
                }
              >
                <Icon name={p} />
                <span>
                  {
                    { tasks: "任务", files: "导入文件", settings: "应用设置" }[
                      p
                    ]
                  }
                </span>
                {p === "tasks" && <b>{data.config.tasks.length}</b>}
              </button>
            ))}
          </nav>
          <div className="sidebar-bottom">
            <div className="local-state">
              <span className="live-dot" />
              <span className="muted">{totalActive} 个运行实例</span>
            </div>
            <small>Script Hub · v0.1.0</small>
          </div>
        </aside>
        <main
          className="main-content"
          inert={!!taskDialog || !!confirm || !!pendingConfig}
        >
          <header className="page-header" data-tauri-drag-region>
            <div>
              <div className="eyebrow">
                SCRIPT HUB /{" "}
                {
                  {
                    tasks: "WORKSPACE",
                    files: "FILES",
                    settings: "PREFERENCES",
                  }[page]
                }
              </div>
              <h1>
                {
                  {
                    tasks: "让本机任务，井然有序。",
                    files: "管理你的导入文件。",
                    settings: "按你的习惯工作。",
                  }[page]
                }
              </h1>
              <p>
                {
                  {
                    tasks: "保存命令、管理运行，专注正在做的事。",
                    files: "导入文件拥有独立副本，可供多个任务引用。",
                    settings: "配置保存在用户主目录，随时可以直接编辑。",
                  }[page]
                }
              </p>
            </div>
            {page === "tasks" && (
              <Button
                icon="plus"
                variant="primary"
                size="page"
                disabled={!canCreateTask}
                onClick={() => guarded(newTask)}
              >
                新增任务
              </Button>
            )}
            {page === "files" && (
              <Button
                icon="plus"
                variant="primary"
                size="page"
                disabled={busy || !desktop || !!data.configError}
                onClick={() => void work(() => importFile())}
              >
                导入文件
              </Button>
            )}
          </header>
          <div className="page-body" key={page}>
            {!desktop && (
              <div className="banner info">
                当前为浏览器界面预览。运行、文件和配置管理请使用桌面应用：
                <code>pnpm tauri dev</code>
              </div>
            )}
            {(data.configError || data.runtimeError) && (
              <div className="banner danger">
                <strong>
                  {data.configError ? "配置未加载" : "运行管理需要处理"}
                </strong>
                <span>{data.configError ?? data.runtimeError}</span>
                {data.configError && (
                  <div className="button-row">
                    <Button
                      disabled={busy}
                      onClick={() =>
                        void work(async () => {
                          await call("recover_config", { backup: true });
                          await refresh();
                        })
                      }
                    >
                      恢复有效备份
                    </Button>
                    <Button
                      disabled={busy}
                      onClick={() =>
                        setConfirm({
                          title: "新建配置？",
                          body: "损坏的原配置会另存保留，当前配置将重置为空。",
                          label: "新建空配置",
                          action: () =>
                            void work(async () => {
                              await call("recover_config", { backup: false });
                              await refresh();
                              loadTask(null);
                            }),
                        })
                      }
                    >
                      新建空配置
                    </Button>
                    <Button onClick={reload}>重试加载</Button>
                  </div>
                )}
              </div>
            )}
            {data.runs
              .filter((r) => r.status === "abnormal")
              .map((r) => (
                <div className="banner danger" key={r.id}>
                  <strong>需要核查的运行实例：{r.taskName}</strong>
                  <span>{r.message ?? "请确认旧实例结束后再运行。"}</span>
                  <div className="button-row">
                    <Button
                      disabled={busy}
                      onClick={() =>
                        void work(async () => {
                          await call("cleanup_run", {
                            runId: r.id,
                            terminate: false,
                          });
                          await refresh();
                        })
                      }
                    >
                      重新核查
                    </Button>
                    <Button
                      disabled={busy}
                      onClick={() =>
                        setConfirm({
                          title: "清理遗留运行实例？",
                          body: `将结束「${r.taskName}」仍存活的受管进程，并核查结束状态。`,
                          label: "清理并核查",
                          action: () =>
                            void work(async () => {
                              await call("cleanup_run", {
                                runId: r.id,
                                terminate: true,
                              });
                              await refresh();
                            }),
                        })
                      }
                    >
                      清理并核查
                    </Button>
                  </div>
                </div>
              ))}
            {data.appError && (
              <div className="banner info">{data.appError}</div>
            )}
            {error && (
              <div role="alert" className="banner danger">
                <span>{error}</span>
                <Button
                  size="icon"
                  className="banner-close"
                  aria-label="关闭错误"
                  onClick={() => setError("")}
                >
                  <Icon name="close" />
                </Button>
              </div>
            )}
            {notice && (
              <div role="status" className="banner success">
                <Icon name="check" size={16} />
                <span>{notice}</span>
                <Button
                  size="icon"
                  className="banner-close"
                  aria-label="关闭提示"
                  onClick={() => setNotice("")}
                >
                  <Icon name="close" />
                </Button>
              </div>
            )}
            {page === "tasks" && (
              <>
                <div className="metrics">
                  <MetricCard
                    icon="tasks"
                    label="全部任务"
                    value={data.config.tasks.length}
                    detail="个任务"
                  />
                  <MetricCard
                    icon="play"
                    label="运行中"
                    value={
                      data.runs.filter((r) => r.status === "running").length
                    }
                    detail="个运行实例"
                  />
                  <MetricCard
                    icon="clock"
                    label="需要关注"
                    value={
                      data.config.tasks.filter((t) =>
                        ["failed", "abnormal"].includes(
                          data.runs.find((r) => r.taskId === t.id)?.status ??
                            "",
                        ),
                      ).length
                    }
                    detail="个任务"
                  />
                  <MetricCard
                    icon="files"
                    label="导入文件"
                    value={data.config.files.length}
                    detail="个本机副本"
                  />
                </div>
                <div className="workspace">
                  <Card className="task-list">
                    <div className="section-title">
                      <h2>我的任务</h2>
                      <div
                        className={`task-search${searchOpen ? " expanded" : ""}`}
                      >
                        {searchOpen && (
                          <input
                            id="task-search"
                            autoFocus
                            type="search"
                            aria-label="搜索任务"
                            placeholder="搜索任务名称或命令…"
                            value={query}
                            onChange={(e) => setQuery(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === "Escape") {
                                e.preventDefault();
                                setSearchOpen(false);
                                setQuery("");
                                searchToggleRef.current?.focus();
                              }
                            }}
                          />
                        )}
                        <Button
                          ref={searchToggleRef}
                          type="button"
                          size="icon"
                          variant="muted-icon"
                          className="task-search-toggle"
                          aria-label={searchOpen ? "收起搜索" : "搜索任务"}
                          title={searchOpen ? "收起搜索" : "搜索任务"}
                          aria-expanded={searchOpen}
                          aria-controls={searchOpen ? "task-search" : undefined}
                          onClick={() => {
                            setSearchOpen(!searchOpen);
                            if (searchOpen) setQuery("");
                          }}
                        >
                          <Icon
                            name={searchOpen ? "close" : "search"}
                            size={16}
                          />
                        </Button>
                      </div>
                    </div>
                    <div
                      className="task-rows"
                      role="list"
                      aria-label="任务列表"
                    >
                      {data.config.tasks
                        .filter((t) =>
                          `${t.name} ${JSON.stringify(t.command)}`
                            .toLowerCase()
                            .includes(query.toLowerCase()),
                        )
                        .map((task) => {
                          const latestRun = data.runs.find(
                            (r) => r.taskId === task.id,
                          );
                          return (
                            <div
                              className="task-row"
                              role="listitem"
                              key={task.id}
                            >
                              <div className="task-row-content">
                                <div className="task-row-top">
                                  <span className="task-symbol">
                                    <Icon name="terminal" size={16} />
                                  </span>
                                  <strong>{task.name}</strong>
                                  {task.autoStart && (
                                    <span
                                      title="任务自动启动"
                                      className="auto-mark"
                                    >
                                      A
                                    </span>
                                  )}
                                </div>
                                <code>
                                  {task.command.mode === "direct"
                                    ? task.command.text
                                    : task.command.parts
                                        .map((p) =>
                                          p.kind === "file"
                                            ? data.config.files.find(
                                                (f) => f.id === p.file_id,
                                              )?.name
                                            : p.value,
                                        )
                                        .join(" ")}
                                </code>
                              </div>
                              <div className="task-row-bottom">
                                <Badge status={latestRun?.status} />
                                <span>
                                  {latestRun
                                    ? duration(latestRun, clock)
                                    : "等待首次运行"}
                                </span>
                              </div>
                              <div className="task-row-actions button-row">
                                <Button
                                  icon="edit"
                                  disabled={busy}
                                  onClick={() => openTaskDialog(task, "edit")}
                                  aria-label={`编辑 ${task.name}`}
                                >
                                  编辑
                                </Button>
                                <Button
                                  icon="terminal"
                                  disabled={busy}
                                  onClick={() => openTaskDialog(task, "logs")}
                                  aria-label={`查看 ${task.name} 的日志`}
                                >
                                  查看日志
                                </Button>
                              </div>
                            </div>
                          );
                        })}
                      {data.config.tasks.length === 0 && (
                        <div className="list-empty">
                          <Icon name="tasks" size={30} />
                          <strong>从第一个任务开始</strong>
                          <p>一条命令，一处管理。</p>
                          <Button
                            icon="plus"
                            disabled={!canCreateTask}
                            onClick={newTask}
                          >
                            新增任务
                          </Button>
                        </div>
                      )}
                      {data.config.tasks.length > 0 &&
                        !data.config.tasks.some((t) =>
                          `${t.name} ${JSON.stringify(t.command)}`
                            .toLowerCase()
                            .includes(query.toLowerCase()),
                        ) && <p className="empty-small">没有匹配的任务</p>}
                    </div>
                  </Card>
                </div>
              </>
            )}
            {page === "files" && (
              <Card className="files-panel">
                <div className="section-title">
                  <h2>
                    导入文件{" "}
                    <span className="muted">{data.config.files.length}</span>
                  </h2>
                  <span className="muted">独立身份 · 同名文件互不覆盖</span>
                </div>
                <p className="helper">
                  仅复制选择的普通文件。依赖与运行环境由你准备，可将任务工作目录指向现有项目。
                </p>
                {data.config.files.length ? (
                  <div className="file-list">
                    {data.config.files.map((file) => {
                      const refs = data.config.tasks.filter(
                        (t) =>
                          t.command.mode === "composed" &&
                          t.command.parts.some(
                            (p) => p.kind === "file" && p.file_id === file.id,
                          ),
                      );
                      return (
                        <div className="file-row" key={file.id}>
                          <div className="file-icon">
                            <Icon name="files" size={22} />
                          </div>
                          <div className="file-info">
                            <strong>{file.name}</strong>
                            <code>{file.id}</code>
                            <span>
                              {refs.length
                                ? `引用任务：${refs.map((t) => t.name).join("、")}`
                                : "尚无任务引用"}
                            </span>
                          </div>
                          <div className="button-row">
                            <Button onClick={() => openDirectory(file.id)}>
                              所在目录
                            </Button>
                            <Button
                              disabled={busy}
                              onClick={() =>
                                setConfirm({
                                  title: "替换导入文件？",
                                  body: `「${file.name}」的内容将被替换，保留原有存储路径和任务引用。`,
                                  label: "选择替换文件",
                                  action: () =>
                                    void work(() => importFile(file.id)),
                                })
                              }
                            >
                              替换
                            </Button>
                            <Button
                              icon="trash"
                              aria-label={`删除文件 ${file.name}`}
                              disabled={busy || refs.length > 0}
                              onClick={() =>
                                setConfirm({
                                  title: "删除导入文件？",
                                  body: `将删除软件管理的「${file.name}」副本。`,
                                  label: "删除文件",
                                  action: () =>
                                    void work(async () => {
                                      await call("delete_file", {
                                        fileId: file.id,
                                      });
                                      await refresh();
                                    }),
                                })
                              }
                            />
                          </div>
                        </div>
                      );
                    })}
                  </div>
                ) : (
                  <div className="large-empty">
                    <Icon name="files" size={40} />
                    <h2>把文件放进工作空间</h2>
                    <p>
                      Python、JavaScript
                      或其他普通文件，都可以导入并供任务引用。
                    </p>
                    <Button
                      icon="plus"
                      variant="primary"
                      disabled={busy || !desktop}
                      onClick={() => void work(() => importFile())}
                    >
                      选择文件导入
                    </Button>
                  </div>
                )}
              </Card>
            )}
            {isSettingsPage && (
              <div className="settings-grid">
                <Card className="settings-card">
                  <h2>启动与常驻</h2>
                  <label className="setting-row">
                    <div>
                      <strong>应用开机启动</strong>
                      <p>
                        当前用户登录系统后隐藏到托盘，仍会触发任务自动启动。
                      </p>
                    </div>
                    <input
                      type="checkbox"
                      className="switch"
                      checked={data.config.settings.launchAtLogin}
                      disabled={!desktop || !!data.configError}
                      onChange={(event) => {
                        const launchAtLogin = event.currentTarget.checked;
                        void work(
                          () =>
                            persist({
                              ...data.config,
                              settings: { launchAtLogin },
                            }),
                        );
                      }}
                    />
                  </label>
                  <label className="setting-row">
                    <div>
                      <strong>关闭窗口时隐藏到托盘</strong>
                      <p>
                        运行中的任务继续执行。明确退出会先停止所有受管进程。
                      </p>
                    </div>
                    <input
                      type="checkbox"
                      role="switch"
                      className="switch"
                      checked
                      disabled
                    />
                  </label>
                  <Button
                    variant="subtle-danger"
                    className="settings-quit"
                    disabled={!desktop}
                    onClick={() =>
                      setConfirm({
                        title: "停止任务并退出？",
                        body: "将先请求所有任务正常结束，最多等待 5 秒，再强制结束受管进程树。",
                        label: "停止任务并退出",
                        action: () =>
                          void work(async () => {
                            await call("quit_app");
                          }),
                      })
                    }
                  >
                    停止任务并退出软件
                  </Button>
                </Card>
                <Card className="settings-card">
                  <h2>配置与数据</h2>
                  <label className="field">
                    <span>数据根目录</span>
                    <code className="path-box">{data.root}</code>
                  </label>
                  <p className="helper">
                    配置为 config.json，上一个有效版本为
                    config.backup.json。导入文件、任务工作目录和日志分别保存。
                  </p>
                  <div className="button-row">
                    <Button
                      icon="files"
                      disabled={!desktop}
                      onClick={() => openDirectory()}
                    >
                      打开配置目录
                    </Button>
                    <Button
                      icon="refresh"
                      disabled={!desktop || totalActive > 0}
                      onClick={() => guarded(reload)}
                    >
                      重新加载配置
                    </Button>
                  </div>
                  <p className="helper">
                    全部任务停止后才可重新加载；重新加载不会触发任务自动启动。
                  </p>
                </Card>
                <Card className="settings-card">
                  <h2>运行与历史</h2>
                  <div className="setting-row">
                    <span>每个任务保留的运行记录</span>
                    <strong>20 次</strong>
                  </div>
                  <div className="setting-row">
                    <span>每次运行的输出上限</span>
                    <strong>10 MB</strong>
                  </div>
                  <div className="setting-row">
                    <span>正常停止等待时间</span>
                    <strong>5 秒</strong>
                  </div>
                  <p className="helper">
                    不自动安装运行环境与依赖，不自动重启失败任务。任务启动失败的具体原因会保存在运行记录中。
                  </p>
                </Card>
              </div>
            )}
          </div>
        </main>
      </div>
      <Modal
        open={!!taskDialog}
        titleId="task-dialog-title"
        onClose={closeTaskDialog}
        onExited={() => {
          if (!taskDialog) loadTask(null);
        }}
        backdropClose
        active={!confirm && !pendingConfig}
        backdropClassName="task-dialog-backdrop"
        className={`task-modal ${taskDialog === "logs" ? "logs-modal" : "editor-modal"}`}
      >
        <Button
          size="icon"
          className="modal-close"
          icon="close"
          aria-label="关闭弹窗"
          disabled={busy}
          onClick={closeTaskDialog}
        />
        {error && (
          <div className="banner danger" role="alert">
            {error}
          </div>
        )}
        {notice && (
          <div className="banner info" role="status">
            {notice}
          </div>
        )}
        {taskDialog === "edit" && draft && (
          <section className="editor">
            <div className="section-title">
              <div>
                <div className="eyebrow">TASK CONFIGURATION</div>
                <h2 id="task-dialog-title">
                  {data.config.tasks.some((task) => task.id === draft.id)
                    ? "编辑任务"
                    : "新增任务"}{" "}
                  · {draft.name || "新任务"}{" "}
                  {dirty && <span className="unsaved">未保存</span>}
                </h2>
              </div>
              <div className="button-row">
                <Button
                  icon="copy"
                  title="复制为独立任务"
                  disabled={busy || !draft.name}
                  onClick={() => void work(duplicateTask)}
                >
                  复制
                </Button>
                <Button
                  icon="trash"
                  size="icon"
                  variant="subtle-danger"
                  aria-label="删除任务"
                  disabled={busy || !!running}
                  onClick={() =>
                    setConfirm({
                      title: "删除此任务？",
                      body: `将删除「${draft.name || "新任务"}」的任务定义，导入文件和运行历史会保留。`,
                      label: "删除任务",
                      action: () =>
                        void work(async () => {
                          if (data.config.tasks.some((t) => t.id === draft.id))
                            await persist({
                              ...data.config,
                              tasks: data.config.tasks.filter(
                                (t) => t.id !== draft.id,
                              ),
                            });
                          setTaskDialog(null);
                        }),
                    })
                  }
                />
              </div>
            </div>
            {!!running && (
              <div className="inline-note">
                运行实例使用启动时的配置快照。编辑并保存后，下次运行生效。
              </div>
            )}
            <div className="form-grid">
              <label className="field">
                <span>
                  任务名称 <b>*</b>
                </span>
                <input
                  value={draft.name}
                  placeholder="例如：本地开发服务"
                  onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                />
              </label>
              <label className="check-field">
                <input
                  type="checkbox"
                  checked={draft.autoStart}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      autoStart: e.target.checked,
                    })
                  }
                />
                软件启动时自动运行此任务
              </label>
            </div>
            <div className="field">
              <span>命令编辑方式</span>
              <div className="segmented">
                {(["direct", "composed"] as const).map((mode) => (
                  <button
                    key={mode}
                    className={draft.command.mode === mode ? "selected" : ""}
                    onClick={() => {
                      if (mode !== draft.command.mode)
                        setConfirm({
                          title: "切换命令编辑方式？",
                          body: "两种方式不自动转换，切换会清空当前命令内容。",
                          label: "切换方式",
                          action: () =>
                            setDraft({
                              ...draft,
                              command:
                                mode === "direct"
                                  ? { mode, text: "" }
                                  : {
                                      mode,
                                      parts: [
                                        {
                                          kind: "program",
                                          value: "",
                                        },
                                      ],
                                    },
                            }),
                        });
                    }}
                  >
                    {mode === "direct" ? "完整命令" : "组合编辑"}
                  </button>
                ))}
              </div>
            </div>
            {draft.command.mode === "direct" ? (
              <label className="field">
                <span>完整终端命令</span>
                <textarea
                  className="code-input"
                  rows={3}
                  value={draft.command.text}
                  placeholder={"例如：node server.js --port ${port}"}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      command: {
                        mode: "direct",
                        text: e.target.value,
                      },
                    })
                  }
                />
              </label>
            ) : (
              <div className="parts">
                <div className="field">
                  <span>
                    命令组成项 <small>按顺序用空格连接为一条命令</small>
                  </span>
                </div>
                {draft.command.parts.map((part, index) => (
                  <div className="part-row" key={index}>
                    <span className="part-number">
                      {String(index + 1).padStart(2, "0")}
                    </span>
                    {index === 0 ? (
                      <span className="part-label">程序</span>
                    ) : (
                      <select
                        aria-label={`第 ${index + 1} 项类型`}
                        value={part.kind}
                        onChange={(e) =>
                          updatePart(
                            index,
                            e.target.value === "file"
                              ? {
                                  kind: "file",
                                  file_id: data.config.files[0]?.id ?? "",
                                }
                              : { kind: "text", value: "" },
                          )
                        }
                      >
                        <option value="text">文本</option>
                        <option value="file">文件引用</option>
                      </select>
                    )}
                    {part.kind === "file" ? (
                      <select
                        className="part-value"
                        aria-label={`第 ${index + 1} 项文件`}
                        value={part.file_id}
                        onChange={(e) =>
                          updatePart(index, {
                            kind: "file",
                            file_id: e.target.value,
                          })
                        }
                      >
                        <option value="">选择导入文件</option>
                        {data.config.files.map((f) => (
                          <option key={f.id} value={f.id}>
                            {f.name} · {f.id.slice(0, 6)}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        className="part-value code-input"
                        aria-label={
                          index === 0 ? "运行程序" : `第 ${index + 1} 项文本`
                        }
                        value={part.value}
                        placeholder={
                          index === 0
                            ? "python、node 或程序路径"
                            : "自由文本 / 参数 / ${变量}"
                        }
                        onChange={(e) =>
                          updatePart(index, {
                            ...part,
                            value: e.target.value,
                          })
                        }
                      />
                    )}{" "}
                    {index > 0 && (
                      <div className="part-actions">
                        <button
                          title="上移"
                          aria-label={`上移第 ${index + 1} 项`}
                          disabled={index === 1}
                          onClick={() => movePart(index, -1)}
                        >
                          ↑
                        </button>
                        <button
                          title="下移"
                          aria-label={`下移第 ${index + 1} 项`}
                          disabled={
                            draft.command.mode !== "composed" ||
                            index === draft.command.parts.length - 1
                          }
                          onClick={() => movePart(index, 1)}
                        >
                          ↓
                        </button>
                        <button
                          title="移除"
                          aria-label={`移除第 ${index + 1} 项`}
                          onClick={() => {
                            if (draft.command.mode === "composed")
                              setDraft({
                                ...draft,
                                command: {
                                  mode: "composed",
                                  parts: draft.command.parts.filter(
                                    (_, i) => i !== index,
                                  ),
                                },
                              });
                          }}
                        >
                          ×
                        </button>
                      </div>
                    )}
                  </div>
                ))}
                <Button
                  icon="plus"
                  variant="text"
                  size="text"
                  onClick={() => {
                    if (draft.command.mode === "composed")
                      setDraft({
                        ...draft,
                        command: {
                          mode: "composed",
                          parts: [
                            ...draft.command.parts,
                            { kind: "text", value: "" },
                          ],
                        },
                      });
                  }}
                >
                  添加组成项
                </Button>
              </div>
            )}
            <label className="field">
              <span>工作目录</span>
              <div className="input-action">
                <input
                  className="code-input"
                  value={draft.workingDirectory}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      workingDirectory: e.target.value,
                    })
                  }
                />
                <Button icon="files" onClick={() => void work(chooseDirectory)}>
                  选择
                </Button>
              </div>
            </label>
            <details className="environment">
              <summary>
                任务环境变量{" "}
                <span className="muted">
                  {Object.keys(draft.environment).length} 项 ·
                  覆盖继承的系统环境
                </span>
              </summary>
              <EnvironmentEditor
                value={draft.environment}
                onChange={(environment) => setDraft({ ...draft, environment })}
              />
            </details>
            <div className="variables-block">
              <div className="section-title">
                <h3>输入变量</h3>
                <span className="muted">同名共享赋值 · 默认记住上次输入</span>
              </div>
              {names.length ? (
                names.map((name) => {
                  const variable = draft.variables[name] ?? {
                    value: null,
                    allowEmpty: false,
                  };
                  return (
                    <div className="variable-row" key={name}>
                      <label>
                        <code>{"${" + name + "}"}</code>
                        <input
                          aria-label={`输入变量 ${name}`}
                          value={variable.value ?? ""}
                          placeholder={
                            variable.allowEmpty ? "允许空值" : "必填，尚未赋值"
                          }
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              variables: {
                                ...draft.variables,
                                [name]: {
                                  ...variable,
                                  value: e.target.value,
                                },
                              },
                            })
                          }
                        />
                      </label>
                      <label className="check-inline">
                        <input
                          type="checkbox"
                          checked={variable.allowEmpty}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              variables: {
                                ...draft.variables,
                                [name]: {
                                  ...variable,
                                  allowEmpty: e.target.checked,
                                },
                              },
                            })
                          }
                        />
                        允许空值
                      </label>
                    </div>
                  );
                })
              ) : (
                <p className="helper">
                  在命令、工作目录或环境变量值中写入 <code>{"${名称}"}</code>
                  ，这里会自动生成输入框。
                </p>
              )}
              <p className="helper">
                变量按字面文本替换，请自行添加必要引号。
                <code>{"$${名称}"}</code> 表示字面量占位符。
              </p>
            </div>
            <div className="command-preview">
              <div className="section-title">
                <span>
                  <Icon name="terminal" size={15} /> 最终命令预览
                </span>
                <span className="muted">
                  {missing.length
                    ? `${missing.length} 个变量待填写`
                    : "运行前确认"}
                </span>
              </div>
              <pre>
                {previewError || preview?.command || "填写命令后在此预览"}
              </pre>
              <div className="preview-context">
                <span
                  className="preview-working-directory"
                  title={preview?.workingDirectory}
                >
                  工作目录：{preview?.workingDirectory ?? "计算中…"}
                </span>
                {preview && Object.keys(preview.environment).length > 0 && (
                  <span>
                    环境覆盖：
                    {Object.entries(preview.environment)
                      .map(([k, v]) => `${k}=${v}`)
                      .join(" · ")}
                  </span>
                )}
              </div>
            </div>
            <div className="editor-footer">
              <Button disabled={busy} onClick={closeTaskDialog}>
                取消
              </Button>
              <Button
                disabled={busy || !dirty || !!data.configError}
                onClick={() =>
                  void work(async () => {
                    await saveTask();
                    setTaskDialog(null);
                  })
                }
              >
                保存任务
              </Button>
              <div className="button-row">
                <Button
                  icon="refresh"
                  disabled={
                    busy ||
                    !running ||
                    missing.length > 0 ||
                    running.status === "abnormal"
                  }
                  onClick={() => void work(() => taskAction("restart"))}
                >
                  重启
                </Button>
                {running ? (
                  <Button
                    icon="stop"
                    variant="stop"
                    disabled={
                      busy ||
                      running.status === "stopping" ||
                      running.status === "abnormal"
                    }
                    onClick={() => void work(() => taskAction("stop"))}
                  >
                    {running.status === "stopping" ? "正在停止…" : "停止任务"}
                  </Button>
                ) : (
                  <Button
                    icon="play"
                    variant="primary"
                    disabled={
                      busy ||
                      !draft.name.trim() ||
                      missing.length > 0 ||
                      !!previewError ||
                      !preview?.command.trim() ||
                      !!data.configError ||
                      !!data.runtimeError
                    }
                    onClick={() => void work(() => taskAction("start"))}
                  >
                    {dirty ? "保存并启动" : "启动任务"}
                  </Button>
                )}
              </div>
            </div>
          </section>
        )}
        {taskDialog === "logs" && (
          <section className="output-panel">
            <div className="section-title">
              <h2 id="task-dialog-title">
                <Icon name="terminal" size={17} />
                {draft?.name} · 运行日志
              </h2>
              <div className="button-row">
                <Button
                  icon="copy"
                  disabled={!output.length}
                  onClick={() =>
                    void work(async () => {
                      await navigator.clipboard.writeText(
                        output.map((o) => o.text).join(""),
                      );
                      setNotice("已复制本次运行输出。");
                    })
                  }
                >
                  复制
                </Button>
                <Button
                  disabled={busy || !run}
                  onClick={() =>
                    void work(async () => {
                      await call("clear_output", { runId: run!.id });
                      setOutput([]);
                      await refresh();
                    })
                  }
                >
                  清空本次
                </Button>
              </div>
            </div>
            <div className="output-toolbar">
              <select
                aria-label="运行历史"
                value={run?.id ?? ""}
                onChange={(e) => setChosenRun(e.target.value)}
              >
                <option value="" disabled>
                  选择运行记录
                </option>
                {taskRuns.map((r) => (
                  <option value={r.id} key={r.id}>
                    {dateTime(r.startedAt)} · {statusText[r.status]}
                  </option>
                ))}
              </select>
              <select
                aria-label="输出筛选"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              >
                <option value="all">全部输出</option>
                <option value="stdout">标准输出</option>
                <option value="stderr">错误输出</option>
              </select>
              <label className="check-inline">
                <input
                  type="checkbox"
                  checked={follow}
                  onChange={(e) => setFollow(e.target.checked)}
                />
                跟随输出
              </label>
            </div>
            {run && (
              <div className="run-info">
                <Badge status={run.status} />
                <span>开始于 {dateTime(run.startedAt)}</span>
                <span>
                  <Icon name="clock" size={13} />
                  {duration(run, clock)}
                </span>
                {run.endedAt && <span>结束于 {dateTime(run.endedAt)}</span>}
                {run.exitCode !== null && <span>退出码 {run.exitCode}</span>}
              </div>
            )}
            {run?.message && (
              <div
                className={`banner ${["failed", "abnormal"].includes(run.status) ? "danger" : "info"}`}
              >
                {run.message}
              </div>
            )}
            {run?.status === "abnormal" && (
              <div className="button-row recovery-actions">
                <Button
                  disabled={busy}
                  onClick={() =>
                    void work(async () => {
                      await call("cleanup_run", {
                        runId: run.id,
                        terminate: false,
                      });
                      await refresh();
                    })
                  }
                >
                  重新核查
                </Button>
                <Button
                  disabled={busy}
                  onClick={() =>
                    setConfirm({
                      title: "清理遗留运行实例？",
                      body: "将结束该运行实例仍存活的受管进程，并核查结束状态。",
                      label: "清理并核查",
                      action: () =>
                        void work(async () => {
                          await call("cleanup_run", {
                            runId: run.id,
                            terminate: true,
                          });
                          await refresh();
                        }),
                    })
                  }
                >
                  清理并核查
                </Button>
              </div>
            )}
            {run?.truncated && (
              <p className="truncate-note">
                输出已超过 10 MB，较早内容已丢弃。
              </p>
            )}
            <div
              className="terminal-output"
              ref={outputRef}
              role="log"
              aria-label="运行输出内容"
            >
              {output.length ? (
                output
                  .filter((o) => filter === "all" || filter === o.stream)
                  .map((o, i) => (
                    <span
                      key={i}
                      className={o.stream}
                      title={`${o.stream} · ${dateTime(o.at)}`}
                    >
                      {o.text}
                    </span>
                  ))
              ) : (
                <div className="terminal-empty">
                  <Icon name="terminal" size={24} />
                  <span>
                    {run
                      ? "本次运行暂无输出"
                      : "任务启动后，标准输出与错误输出会显示在这里"}
                  </span>
                </div>
              )}
            </div>
            <div className="output-footer">
              <span>最近 20 次运行 · 每次最多 10 MB</span>
              <button
                className="link-button"
                disabled={busy || !taskRuns.length || !!running}
                onClick={() =>
                  setConfirm({
                    title: "清除全部运行历史？",
                    body: "将永久删除此任务的全部运行记录与输出日志。",
                    label: "清除全部历史",
                    action: () =>
                      void work(async () => {
                        await call("clear_history", {
                          taskId: selected,
                        });
                        await refresh();
                        setOutput([]);
                      }),
                  })
                }
              >
                清除全部历史
              </button>
            </div>
            {run && (
              <details className="snapshot-details">
                <summary>查看启动时的配置快照</summary>
                <pre>
                  {JSON.stringify(
                    { task: run.snapshot, executed: run.preview },
                    null,
                    2,
                  )}
                </pre>
              </details>
            )}
          </section>
        )}
      </Modal>
      <Modal
        open={!!confirm}
        titleId="confirm-title"
        onClose={closeConfirm}
        backdropClose
        active={!pendingConfig}
      >
        {confirm && (
          <>
            <h2 id="confirm-title">{confirm.title}</h2>
            <p>{confirm.body}</p>
            <div className="button-row">
              <Button autoFocus onClick={closeConfirm}>
                取消
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  const action = confirm.action;
                  setConfirm(null);
                  action();
                }}
              >
                {confirm.label ?? "继续"}
              </Button>
            </div>
          </>
        )}
      </Modal>
      <Modal
        open={!!pendingConfig}
        titleId="conflict-title"
        onClose={closePendingConfig}
      >
        {pendingConfig && (
          <>
            <div className="eyebrow">CONFIGURATION CONFLICT</div>
            <h2 id="conflict-title">配置已在外部修改</h2>
            <p>
              保存已停止。你可以加载磁盘上的版本，或用本次准备保存的配置覆盖。覆盖前会备份磁盘版本。
            </p>
            <p className="helper">加载磁盘版本需要先停止全部任务。</p>
            <div className="button-row">
              <Button disabled={busy} onClick={() => setPendingConfig(null)}>
                取消
              </Button>
              <Button disabled={busy || totalActive > 0} onClick={reload}>
                加载磁盘版本
              </Button>
              <Button
                variant="primary"
                disabled={busy}
                onClick={() =>
                  void work(async () => {
                    const config = pendingConfig;
                    await persist(config, true);
                    const task = config.tasks.find((t) => t.id === selected);
                    if (task) loadTask(task);
                    setPendingConfig(null);
                    setNotice("已备份磁盘版本并保存配置。");
                  })
                }
              >
                备份并覆盖
              </Button>
            </div>
          </>
        )}
      </Modal>
    </div>
  );
}

function EnvironmentEditor({
  value,
  onChange,
}: {
  value: Record<string, string>;
  onChange: (value: Record<string, string>) => void;
}) {
  const [rows, setRows] = useState<[string, string][]>(() =>
    Object.entries(value),
  );
  const signature = JSON.stringify(value);
  const previous = useRef(signature);
  useEffect(() => {
    if (signature !== previous.current) {
      setRows(Object.entries(value));
      previous.current = signature;
    }
  }, [signature, value]);
  function update(next: [string, string][]) {
    setRows(next);
    const result = Object.fromEntries(next.filter(([key]) => key));
    previous.current = JSON.stringify(result);
    onChange(result);
  }
  return (
    <div className="environment-editor">
      {rows.map(([key, val], index) => (
        <div className="env-row" key={index}>
          <input
            aria-label={`环境变量 ${index + 1} 名称`}
            placeholder="名称，例如 PATH"
            value={key}
            onChange={(e) =>
              update(
                rows.map((row, i) =>
                  i === index ? [e.target.value, val] : row,
                ),
              )
            }
          />
          <input
            aria-label={`环境变量 ${index + 1} 值`}
            placeholder="值，支持 ${变量}"
            value={val}
            onChange={(e) =>
              update(
                rows.map((row, i) =>
                  i === index ? [key, e.target.value] : row,
                ),
              )
            }
          />
          <Button
            icon="trash"
            aria-label={`移除环境变量 ${index + 1}`}
            onClick={() => update(rows.filter((_, i) => i !== index))}
          />
        </div>
      ))}
      {rows.some(
        ([k], i) => k && rows.findIndex(([other]) => other === k) !== i,
      ) && <p className="inline-error">名称重复，仅最后一项会保存，请修改。</p>}
      <Button
        icon="plus"
        variant="text"
        size="text"
        onClick={() => update([...rows, ["", ""]])}
      >
        添加环境变量
      </Button>
    </div>
  );
}
export default App;
