// A tiny standalone test runner that exercises the pure-functional pieces of
// the extension without spinning up VS Code. Every helper is structured so
// its core logic lives in functions that take plain objects, which keeps the
// runner small and lets us avoid pulling in @vscode/test-electron just for CI.
//
// To run extension-host integration tests, switch this file to use
// @vscode/test-electron — the providers/panels are already shaped to be tested
// that way.

import * as assert from "assert";
import { isLikelyTableReference } from "../parsing";
import { toVisGraph } from "../providers/lineageProvider";
import { collectFailing, computeScore } from "../quality";
import { renderHtml } from "../lineageHtml";

interface Test {
  name: string;
  fn: () => void | Promise<void>;
}

const tests: Test[] = [
  {
    name: "isLikelyTableReference rejects SQL keywords",
    fn: () => {
      assert.strictEqual(isLikelyTableReference("from"), false);
      assert.strictEqual(isLikelyTableReference("SELECT"), false);
      assert.strictEqual(isLikelyTableReference("Where"), false);
    },
  },
  {
    name: "isLikelyTableReference accepts plausible identifiers",
    fn: () => {
      assert.strictEqual(isLikelyTableReference("orders"), true);
      assert.strictEqual(isLikelyTableReference("dim_customers"), true);
      assert.strictEqual(isLikelyTableReference("schema.orders"), true);
    },
  },
  {
    name: "isLikelyTableReference rejects two-letter and digit-prefixed words",
    fn: () => {
      assert.strictEqual(isLikelyTableReference("id"), false);
      assert.strictEqual(isLikelyTableReference(""), false);
      assert.strictEqual(isLikelyTableReference("3rd_party"), false);
    },
  },
  {
    name: "toVisGraph maps nodes and edges",
    fn: () => {
      const g = toVisGraph(
        {
          nodes: [
            { id: "a", name: "orders", type: "Table" },
            { id: "b", name: "revenue_dashboard", type: "Dashboard" },
          ],
          downstreamEdges: [{ fromEntity: "a", toEntity: "b" }],
        },
        "a",
      );
      assert.strictEqual(g.nodes.length, 2);
      const root = g.nodes.find((n) => n.id === "a");
      assert.strictEqual(root?.group, "current");
      const dash = g.nodes.find((n) => n.id === "b");
      assert.strictEqual(dash?.group, "dashboard");
      assert.deepStrictEqual(g.edges, [{ from: "a", to: "b", arrows: "to" }]);
    },
  },
  {
    name: "toVisGraph injects root if absent from nodes",
    fn: () => {
      const g = toVisGraph({ nodes: [], downstreamEdges: [] }, "missing-root");
      assert.strictEqual(g.nodes.length, 1);
      assert.strictEqual(g.nodes[0].id, "missing-root");
      assert.strictEqual(g.nodes[0].group, "current");
    },
  },
  {
    name: "computeScore produces 0..100 percentage of passing tests",
    fn: () => {
      assert.strictEqual(computeScore(null), 0);
      assert.strictEqual(computeScore({ data: [] }), 0);
      assert.strictEqual(
        computeScore({
          data: [
            { tests: [{ name: "x", testCaseStatus: "Success" }] },
          ],
        }),
        100,
      );
      assert.strictEqual(
        computeScore({
          data: [
            {
              tests: [
                { name: "a", testCaseStatus: "Success" },
                { name: "b", testCaseStatus: "Failed" },
                { name: "c", testCaseStatus: "Success" },
                { name: "d", testCaseStatus: "Aborted" },
              ],
            },
          ],
        }),
        50,
      );
    },
  },
  {
    name: "collectFailing pulls failed tests with context",
    fn: () => {
      const failures = collectFailing(
        {
          data: [
            {
              tests: [
                { name: "p", testCaseStatus: "Success" },
                { name: "q", testCaseStatus: "Failed", failureReason: "nulls" },
              ],
            },
          ],
        },
        "db.schema.orders",
      );
      assert.strictEqual(failures.length, 1);
      assert.strictEqual(failures[0].testName, "q");
      assert.strictEqual(failures[0].fqn, "db.schema.orders");
      assert.strictEqual(failures[0].message, "nulls");
    },
  },
  {
    name: "renderHtml embeds the FQN safely and includes vis-network",
    fn: () => {
      const html = renderHtml(
        { nodes: [{ id: "a", label: "orders", group: "table", title: "" }], edges: [] },
        "a",
        "db.schema.<orders>",
      );
      assert.ok(html.includes("vis-network"));
      assert.ok(html.includes("db.schema.&lt;orders&gt;"));
      assert.ok(!html.includes("<orders>"));
    },
  },
];

async function main(): Promise<void> {
  let failed = 0;
  for (const t of tests) {
    try {
      await t.fn();
      // eslint-disable-next-line no-console
      console.log(`✓ ${t.name}`);
    } catch (err) {
      failed += 1;
      // eslint-disable-next-line no-console
      console.error(`✗ ${t.name}\n  ${(err as Error).message}`);
    }
  }
  if (failed > 0) {
    process.exitCode = 1;
    // eslint-disable-next-line no-console
    console.error(`\n${failed} test(s) failed`);
  } else {
    // eslint-disable-next-line no-console
    console.log(`\nAll ${tests.length} tests passed`);
  }
}

void main();
