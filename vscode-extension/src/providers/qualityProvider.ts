// TreeView for the MetaTree sidebar.
// Two top-level groups:
//   - Recently Viewed: tables touched by the hover provider, with a DQ score
//   - Failing Tests: failing test cases pulled from /dataQuality/testSuites

import * as vscode from "vscode";
import { OpenMetadataClient, QualityPayload } from "../client";
import { collectFailing, computeScore, FailingTest } from "../quality";

export { collectFailing, computeScore };

interface Recent {
  fqn: string;
  score: number;
  status: "pass" | "warn" | "fail";
}

export class QualityProvider implements vscode.TreeDataProvider<MtItem> {
  private readonly _onDidChangeTreeData = new vscode.EventEmitter<MtItem | undefined | void>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private recent: Recent[] = [];
  private failing: FailingTest[] = [];

  constructor(private readonly getClient: () => OpenMetadataClient | undefined) {}

  refresh(): void {
    this._onDidChangeTreeData.fire();
  }

  recordRecentlyViewed(fqn: string): void {
    const fresh: Recent = { fqn, score: 0, status: "pass" };
    this.recent = [fresh, ...this.recent.filter((r) => r.fqn !== fqn)].slice(0, 10);
    void this.hydrate(fqn);
  }

  private async hydrate(fqn: string): Promise<void> {
    const client = this.getClient();
    if (!client) return;
    try {
      const payload = await client.qualityForFqn(fqn);
      const score = computeScore(payload);
      const status: Recent["status"] = score >= 70 ? "pass" : score >= 40 ? "warn" : "fail";
      const updated = this.recent.find((r) => r.fqn === fqn);
      if (updated) {
        updated.score = score;
        updated.status = status;
      }
      this.failing = [
        ...this.failing.filter((f) => f.fqn !== fqn),
        ...collectFailing(payload, fqn),
      ];
      this.refresh();
    } catch {
      // Surface failures silently; the sidebar still shows what we have.
    }
  }

  getTreeItem(element: MtItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: MtItem): vscode.ProviderResult<MtItem[]> {
    if (!element) {
      return [
        new MtItem("Recently Viewed", vscode.TreeItemCollapsibleState.Expanded, "group:recent"),
        new MtItem("Failing Tests", vscode.TreeItemCollapsibleState.Expanded, "group:failing"),
      ];
    }
    if (element.id === "group:recent") {
      if (this.recent.length === 0) {
        return [new MtItem("(hover a table to begin)", vscode.TreeItemCollapsibleState.None, "info")];
      }
      return this.recent.map((r) => recentItem(r));
    }
    if (element.id === "group:failing") {
      if (this.failing.length === 0) {
        return [new MtItem("(none)", vscode.TreeItemCollapsibleState.None, "info")];
      }
      return this.failing.map((f) => failingItem(f));
    }
    return [];
  }
}

class MtItem extends vscode.TreeItem {
  constructor(
    label: string,
    collapsibleState: vscode.TreeItemCollapsibleState,
    id: string,
  ) {
    super(label, collapsibleState);
    this.id = id;
  }
}

function recentItem(r: Recent): MtItem {
  const icon = r.status === "pass" ? "✅" : r.status === "warn" ? "⚠️" : "❌";
  const item = new MtItem(
    `${icon} ${r.fqn} (${r.score}/100)`,
    vscode.TreeItemCollapsibleState.None,
    `recent:${r.fqn}`,
  );
  item.command = {
    command: "metatree.openLineageForFqn",
    title: "Show Lineage",
    arguments: [r.fqn],
  };
  return item;
}

function failingItem(f: FailingTest): MtItem {
  const item = new MtItem(
    `${f.fqn} — "${f.testName}" FAILED`,
    vscode.TreeItemCollapsibleState.None,
    `fail:${f.fqn}:${f.testName}`,
  );
  item.tooltip = f.message;
  item.command = {
    command: "metatree.openLineageForFqn",
    title: "Show Lineage",
    arguments: [f.fqn],
  };
  return item;
}

