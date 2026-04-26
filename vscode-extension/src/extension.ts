// Activation entrypoint for the MetaTree VS Code extension.
// - Reads metatree.host / metatree.token from workspace settings.
// - Reinitializes the client on configuration change (no reload required).
// - Wires up hover, lineage panel, sidebar, and a status-bar item.

import * as vscode from "vscode";
import { OpenMetadataClient } from "./client";
import { MetaTreeHoverProvider, isLikelyTableReference } from "./providers/hoverProvider";
import { LineagePanel } from "./panels/lineagePanel";
import { QualityProvider } from "./providers/qualityProvider";

let client: OpenMetadataClient | undefined;
let qualityProvider: QualityProvider | undefined;
let statusBar: vscode.StatusBarItem | undefined;

function readConfig(): { host: string; token: string; enabled: boolean } {
  const cfg = vscode.workspace.getConfiguration("metatree");
  return {
    host: cfg.get<string>("host", "").trim(),
    token: cfg.get<string>("token", "").trim(),
    enabled: cfg.get<boolean>("enabled", true),
  };
}

function makeClient(): OpenMetadataClient | undefined {
  const { host, token, enabled } = readConfig();
  if (!enabled || !host || !token) return undefined;
  return new OpenMetadataClient({ host, token });
}

export function activate(context: vscode.ExtensionContext): void {
  client = makeClient();
  if (!client) {
    promptToConfigure();
  }

  qualityProvider = new QualityProvider(() => client);
  context.subscriptions.push(
    vscode.window.registerTreeDataProvider("metatree.assetsView", qualityProvider),
  );

  const hoverProvider = new MetaTreeHoverProvider(
    () => client,
    (fqn) => qualityProvider?.recordRecentlyViewed(fqn),
  );
  for (const language of ["sql", "yaml"]) {
    context.subscriptions.push(
      vscode.languages.registerHoverProvider({ language }, hoverProvider),
    );
  }

  context.subscriptions.push(
    vscode.commands.registerCommand("metatree.showLineage", async () => {
      await runShowLineage(context);
    }),
    vscode.commands.registerCommand("metatree.refreshSidebar", () => {
      qualityProvider?.refresh();
    }),
    vscode.commands.registerCommand("metatree.openInBrowser", async () => {
      await runOpenInBrowser();
    }),
    vscode.commands.registerCommand("metatree.openLineageForFqn", async (fqn: string) => {
      await openLineage(context, fqn);
    }),
  );

  statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  statusBar.text = "$(database) MetaTree";
  statusBar.tooltip = "Open MetaTree sidebar";
  statusBar.command = "metatree.assetsView.focus";
  statusBar.show();
  context.subscriptions.push(statusBar);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("metatree")) {
        client = makeClient();
        qualityProvider?.refresh();
      }
    }),
  );
}

export function deactivate(): void {
  // intentionally empty
}

async function promptToConfigure(): Promise<void> {
  const action = await vscode.window.showInformationMessage(
    "MetaTree: set `metatree.host` and `metatree.token` to enable hover + lineage.",
    "Configure MetaTree",
  );
  if (action === "Configure MetaTree") {
    void vscode.commands.executeCommand("workbench.action.openSettings", "metatree");
  }
}

async function runShowLineage(context: vscode.ExtensionContext): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const range = editor.document.getWordRangeAtPosition(editor.selection.active, /[A-Za-z_][\w.]*/);
  if (!range) return;
  const word = editor.document.getText(range);
  if (!isLikelyTableReference(word)) {
    void vscode.window.showInformationMessage(`MetaTree: "${word}" doesn't look like a table reference.`);
    return;
  }
  if (!client) {
    promptToConfigure();
    return;
  }
  const hits = await client.search(word.split(".").pop() ?? word);
  const top = hits[0];
  if (!top || top.score <= 0.5) {
    void vscode.window.showInformationMessage(`MetaTree: no catalog match for "${word}".`);
    return;
  }
  await openLineage(context, top.fullyQualifiedName);
}

async function openLineage(context: vscode.ExtensionContext, fqn: string): Promise<void> {
  if (!client) {
    promptToConfigure();
    return;
  }
  await LineagePanel.show(context, client, fqn);
  qualityProvider?.recordRecentlyViewed(fqn);
}

async function runOpenInBrowser(): Promise<void> {
  if (!client) {
    promptToConfigure();
    return;
  }
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const range = editor.document.getWordRangeAtPosition(editor.selection.active, /[A-Za-z_][\w.]*/);
  if (!range) return;
  const word = editor.document.getText(range);
  const hits = await client.search(word.split(".").pop() ?? word);
  const top = hits[0];
  if (!top) {
    void vscode.window.showInformationMessage(`MetaTree: no catalog match for "${word}".`);
    return;
  }
  await vscode.env.openExternal(vscode.Uri.parse(client.entityUrl(top.fullyQualifiedName, top.entityType)));
}
