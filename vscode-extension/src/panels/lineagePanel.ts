// Webview panel that renders an interactive lineage graph using vis-network.
// vis-network is loaded from a CDN; the panel is single-shot, recreated each
// invocation, and posts messages back when the user double-clicks a node.

import * as vscode from "vscode";
import { OpenMetadataClient, LineagePayload } from "../client";
import { toVisGraph } from "../providers/lineageProvider";
import { renderHtml } from "../lineageHtml";

export { renderHtml };

export class LineagePanel {
  static async show(
    context: vscode.ExtensionContext,
    client: OpenMetadataClient,
    fqn: string,
  ): Promise<void> {
    const table = await client.tableByFqn(fqn, "id");
    if (!table) {
      void vscode.window.showWarningMessage(`MetaTree: "${fqn}" not found in the catalog.`);
      return;
    }
    const lineage =
      (await client.lineageById(table.id, 2, 2)) ?? ({} as LineagePayload);

    const panel = vscode.window.createWebviewPanel(
      "metatreeLineage",
      `Lineage: ${fqn}`,
      vscode.ViewColumn.Beside,
      { enableScripts: true, retainContextWhenHidden: true },
    );
    panel.webview.html = renderHtml(toVisGraph(lineage, table.id), table.id, fqn);

    panel.webview.onDidReceiveMessage((msg: { type: string; fqn?: string }) => {
      if (msg.type === "openEntity" && msg.fqn) {
        void vscode.commands.executeCommand("metatree.openLineageForFqn", msg.fqn);
      }
    }, undefined, context.subscriptions);
  }
}

