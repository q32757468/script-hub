// UI test fixture only. Native execution is tested separately in hub.rs.
(() => {
  window.isTauri = true;
  const callbacks = new Map();
  let callbackId = 0;
  let config = {
    version: 1,
    settings: { launchAtLogin: false },
    files: [],
    tasks: [
      {
        id: "example-task",
        name: "本地开发服务",
        command: {
          mode: "direct",
          text: "node server.js --port ${port} $${literal}",
        },
        workingDirectory:
          "C:\\Users\\test\\.script-hub\\workspaces\\example-task",
        environment: {},
        variables: { port: { value: "3000", allowEmpty: false } },
        autoStart: false,
      },
    ],
  };
  const runs = [];
  const output = new Map();
  window.__hubTest = { calls: [], conflict: false };
  function substitute(text, variables) {
    return text.replace(
      /\$\$\{([A-Za-z_][A-Za-z_0-9]*)\}|\$\{([A-Za-z_][A-Za-z_0-9]*)\}/g,
      (_, escaped, name) =>
        escaped
          ? "${" + escaped + "}"
          : (variables[name]?.value ?? "${" + name + "}"),
    );
  }
  window.__TAURI_INTERNALS__ = {
    transformCallback(fn) {
      const id = ++callbackId;
      callbacks.set(id, fn);
      return id;
    },
    unregisterCallback(id) {
      callbacks.delete(id);
    },
    async invoke(command, args = {}) {
      window.__hubTest.calls.push({ command, args: structuredClone(args) });
      switch (command) {
        case "snapshot":
          return structuredClone({
            config,
            runs,
            root: "C:\\Users\\test\\.script-hub",
            configError: null,
            runtimeError: null,
          });
        case "save_config":
          if (window.__hubTest.conflict && !args.overwrite)
            throw new Error("CONFIG_CONFLICT：外部修改");
          config = structuredClone(args.config);
          window.__hubTest.conflict = false;
          return;
        case "preview_command": {
          const task = args.task;
          const command =
            task.command.mode === "direct"
              ? task.command.text
              : task.command.parts
                  .map((p) =>
                    p.kind === "file" ? '"C:\\test\\file.py"' : p.value,
                  )
                  .join(" ");
          return {
            command: substitute(command, task.variables),
            workingDirectory: substitute(task.workingDirectory, task.variables),
            environment: task.environment,
            variableNames: [],
          };
        }
        case "run_task": {
          const task = config.tasks.find((t) => t.id === args.taskId);
          if (args.action === "stop") {
            const run = runs.find((r) => r.taskId === args.taskId);
            run.status = "stopped";
            run.endedAt = Date.now();
            return;
          }
          const run = {
            id: crypto.randomUUID(),
            taskId: task.id,
            taskName: task.name,
            snapshot: structuredClone(task),
            preview: {
              command: task.command.text,
              workingDirectory: task.workingDirectory,
              environment: task.environment,
            },
            status: "running",
            startedAt: Date.now(),
            endedAt: null,
            exitCode: null,
            pid: 1234,
            message: null,
            truncated: false,
          };
          runs.unshift(run);
          output.set(
            run.id,
            JSON.stringify({
              stream: "stdout",
              text: "服务已启动\nListening on port 3000\n",
              at: Date.now(),
            }) +
              "\n" +
              JSON.stringify({
                stream: "stderr",
                text: "示例错误输出\n",
                at: Date.now(),
              }) +
              "\n",
          );
          return;
        }
        case "read_output":
          return output.get(args.runId) ?? "";
        case "clear_output":
          output.set(args.runId, "");
          return;
        case "reload_config":
          return;
        case "plugin:event|listen":
          return args.handler;
        case "plugin:event|unlisten":
          return;
        default:
          throw new Error("UI fixture has no handler for " + command);
      }
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
})();
