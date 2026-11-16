import { test } from "node:test";
import assert from "node:assert/strict";
import {
  variableNames,
  normalizeTask,
  active,
  duration,
  parseOutput,
  type Task,
} from "../src/domain.ts";
const task: Task = {
  id: "example",
  name: "example",
  autoStart: false,
  command: {
    mode: "direct",
    text: "node ${program} --name ${Name} ${program} $${literal} ${2bad}",
  },
  workingDirectory: "${directory}",
  environment: { VALUE: "${name}" },
  variables: {
    name: { value: "${inner}", allowEmpty: false },
    obsolete: { value: "old", allowEmpty: true },
  },
};
test("variable discovery is case sensitive, shared, escaped and spans supported fields", () => {
  assert.deepEqual(variableNames(task), [
    "Name",
    "directory",
    "name",
    "program",
  ]);
  const normalized = normalizeTask(task);
  assert.equal(normalized.variables.name.value, "${inner}");
  assert.equal(normalized.variables.program.value, null);
  assert.equal(normalized.variables.program.allowEmpty, false);
  assert.equal(normalized.variables.obsolete, undefined);
});
test("file references are excluded and program/text parts are discovered", () => {
  assert.deepEqual(
    variableNames({
      ...task,
      workingDirectory: "workspace",
      environment: {},
      command: {
        mode: "composed",
        parts: [
          { kind: "program", value: "${exe}" },
          { kind: "file", file_id: "${not_a_variable}" },
          { kind: "text", value: "${arg} $${escaped}" },
        ],
      },
    }),
    ["arg", "exe"],
  );
});
test("abnormal and stopping states still reserve the task instance", () => {
  for (const state of ["starting", "running", "stopping", "abnormal"])
    assert.equal(active(state), true);
  for (const state of [
    "succeeded",
    "failed",
    "interrupted",
    "stopped",
    undefined,
  ])
    assert.equal(active(state), false);
});
test("duration freezes at the end time", () => {
  const run = { startedAt: 1000, endedAt: 62000 } as Parameters<
    typeof duration
  >[0];
  assert.equal(duration(run, 1000000), "1 分 1 秒");
});

test("an incomplete output record does not hide the rest of the run", () => {
  const output = parseOutput(
    JSON.stringify({ stream: "stdout", text: "readable", at: 1 }) +
      '\n{"stream":',
  );
  assert.equal(output[0].text, "readable");
  assert.equal(output[1].stream, "stderr");
  assert.match(output[1].text, /损坏/);
});
