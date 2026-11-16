export type Variable = { value: string | null; allowEmpty: boolean };
export type Part =
  | { kind: "program" | "text"; value: string }
  | { kind: "file"; file_id: string };
export type Command =
  { mode: "direct"; text: string } | { mode: "composed"; parts: Part[] };
export type Task = {
  id: string;
  name: string;
  command: Command;
  workingDirectory: string;
  environment: Record<string, string>;
  variables: Record<string, Variable>;
  autoStart: boolean;
};
export type ImportedFile = { id: string; name: string; storedName: string };
export type Config = {
  version: number;
  settings: { launchAtLogin: boolean };
  tasks: Task[];
  files: ImportedFile[];
};
export type Preview = {
  command: string;
  workingDirectory: string;
  environment: Record<string, string>;
  variableNames: string[];
};
export type Run = {
  id: string;
  taskId: string;
  taskName: string;
  snapshot: Task;
  preview: Omit<Preview, "variableNames">;
  status: string;
  startedAt: number;
  endedAt: number | null;
  exitCode: number | null;
  pid: number | null;
  message: string | null;
  truncated: boolean;
};
export type Snapshot = {
  config: Config;
  configError: string | null;
  runtimeError: string | null;
  appError: string | null;
  root: string;
  runs: Run[];
};
export type Output = { stream: "stdout" | "stderr"; text: string; at: number };
export const active = (status?: string) =>
  ["starting", "running", "stopping", "abnormal"].includes(status ?? "");
export const statusText: Record<string, string> = {
  starting: "正在启动",
  running: "运行中",
  stopping: "正在停止",
  abnormal: "需要核查",
  succeeded: "已完成",
  failed: "运行失败",
  stopped: "已停止",
  interrupted: "已中断",
  idle: "未运行",
};
export function variableNames(task: Task): string[] {
  const texts = [task.workingDirectory, ...Object.values(task.environment)];
  if (task.command.mode === "direct") texts.push(task.command.text);
  else
    for (const part of task.command.parts)
      if (part.kind !== "file") texts.push(part.value);
  const names = new Set<string>();
  for (const text of texts) {
    for (const match of text.matchAll(
      /\$\$\{[A-Za-z_][A-Za-z_0-9]*\}|\$\{([A-Za-z_][A-Za-z_0-9]*)\}/g,
    )) {
      if (match[1]) names.add(match[1]);
    }
  }
  return [...names].sort();
}
export function normalizeTask(task: Task): Task {
  return {
    ...task,
    variables: Object.fromEntries(
      variableNames(task).map((name) => [
        name,
        task.variables[name] ?? { value: null, allowEmpty: false },
      ]),
    ),
  };
}
export function duration(run: Run, now = Date.now()): string {
  const s = Math.max(
    0,
    Math.floor(((run.endedAt ?? now) - run.startedAt) / 1000),
  );
  return s < 60
    ? `${s} 秒`
    : s < 3600
      ? `${Math.floor(s / 60)} 分 ${s % 60} 秒`
      : `${Math.floor(s / 3600)} 时 ${Math.floor(s / 60) % 60} 分`;
}
export const dateTime = (t: number) =>
  new Date(t).toLocaleString("zh-CN", { hour12: false });

export function parseOutput(raw: string): Output[] {
  return raw
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      try {
        const entry = JSON.parse(line);
        if (
          (entry.stream === "stdout" || entry.stream === "stderr") &&
          typeof entry.text === "string" &&
          typeof entry.at === "number"
        )
          return entry as Output;
      } catch {
        /* A crash can leave a partial final record. Keep the other records readable. */
      }
      return {
        stream: "stderr",
        text: "[一段输出日志不完整或已损坏]\n",
        at: Date.now(),
      };
    });
}
