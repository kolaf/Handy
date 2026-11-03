import assert from "node:assert";
import {
  reduceRecordingPhase,
  reduceTranscriptionOutcome,
} from "./RecordingStatus";

assert.deepStrictEqual(reduceRecordingPhase("started"), {
  kind: "recording",
  locked: false,
});
assert.deepStrictEqual(reduceRecordingPhase("locked"), {
  kind: "recording",
  locked: true,
});
assert.deepStrictEqual(reduceRecordingPhase("stopped"), {
  kind: "transcribing",
});
assert.deepStrictEqual(reduceRecordingPhase("cancelled"), { kind: "idle" });

assert.deepStrictEqual(reduceTranscriptionOutcome("typed"), {
  kind: "done",
  outcome: "typed",
});
assert.deepStrictEqual(reduceTranscriptionOutcome("empty"), {
  kind: "done",
  outcome: "empty",
});
assert.deepStrictEqual(reduceTranscriptionOutcome("silent"), {
  kind: "done",
  outcome: "silent",
});
assert.deepStrictEqual(reduceTranscriptionOutcome("failed"), {
  kind: "failed",
});

console.log("recordingStatus reducer checks passed");
