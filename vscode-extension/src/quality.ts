// Pure scoring/aggregation helpers for the quality sidebar.
// No `vscode` imports -- safe to require under plain Node.

import { QualityPayload } from "./client";

export interface FailingTest {
  fqn: string;
  testName: string;
  message: string;
}

export function computeScore(payload: QualityPayload | null): number {
  if (!payload?.data) return 0;
  let total = 0;
  let passing = 0;
  for (const suite of payload.data) {
    for (const test of suite.tests ?? []) {
      total += 1;
      const status = (test.testCaseStatus ?? test.status ?? "").toLowerCase();
      if (status === "success" || status === "passed" || status === "pass") {
        passing += 1;
      }
    }
  }
  if (total === 0) return 0;
  return Math.round((passing / total) * 100);
}

export function collectFailing(
  payload: QualityPayload | null,
  fqn: string,
): FailingTest[] {
  const out: FailingTest[] = [];
  if (!payload?.data) return out;
  for (const suite of payload.data) {
    for (const test of suite.tests ?? []) {
      const status = (test.testCaseStatus ?? test.status ?? "").toLowerCase();
      if (status === "failed" || status === "fail") {
        out.push({
          fqn,
          testName: test.name,
          message: test.failureReason ?? test.message ?? "test failed",
        });
      }
    }
  }
  return out;
}
