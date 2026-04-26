// MarkdownString hover provider for SQL/dbt files.
// Caches lookups per word for 5 minutes to keep keystroke latency low.

import * as vscode from "vscode";
import {
  OpenMetadataClient,
  SearchHit,
  TableEntity,
} from "../client";
import { isLikelyTableReference } from "../parsing";

export { isLikelyTableReference };

const CACHE_TTL_MS = 5 * 60 * 1000;

interface CacheEntry {
  expires: number;
  markdown: vscode.MarkdownString | null;
}

export class MetaTreeHoverProvider implements vscode.HoverProvider {
  private cache = new Map<string, CacheEntry>();

  constructor(
    private readonly getClient: () => OpenMetadataClient | undefined,
    private readonly onResolved: (fqn: string) => void,
  ) {}

  async provideHover(
    document: vscode.TextDocument,
    position: vscode.Position,
    token: vscode.CancellationToken,
  ): Promise<vscode.Hover | undefined> {
    const range = document.getWordRangeAtPosition(position, /[A-Za-z_][\w.]*/);
    if (!range) return undefined;
    const word = document.getText(range);
    if (!isLikelyTableReference(word)) return undefined;

    const client = this.getClient();
    if (!client) return undefined;

    const cached = this.cache.get(word);
    if (cached && cached.expires > Date.now()) {
      return cached.markdown ? new vscode.Hover(cached.markdown, range) : undefined;
    }

    let markdown: vscode.MarkdownString | null = null;
    try {
      markdown = await this.lookup(client, word, token);
    } catch (_err) {
      markdown = null;
    }
    this.cache.set(word, { expires: Date.now() + CACHE_TTL_MS, markdown });
    if (!markdown) return undefined;
    return new vscode.Hover(markdown, range);
  }

  private async lookup(
    client: OpenMetadataClient,
    word: string,
    token: vscode.CancellationToken,
  ): Promise<vscode.MarkdownString | null> {
    const short = word.split(".").pop() ?? word;
    const hits: SearchHit[] = await client.search(short, "table_search_index", 3);
    if (token.isCancellationRequested) return null;
    const best = hits[0];
    if (!best || best.score <= 0.5) return null;

    const detail = await client.tableByFqn(best.fullyQualifiedName);
    if (token.isCancellationRequested) return null;
    if (!detail) return null;

    this.onResolved(detail.fullyQualifiedName);
    return renderHover(detail, client.entityUrl(detail.fullyQualifiedName));
  }
}

export function renderHover(t: TableEntity, browserUrl: string): vscode.MarkdownString {
  const md = new vscode.MarkdownString();
  md.isTrusted = true;
  md.supportHtml = true;
  md.appendMarkdown(`**📊 ${t.name}** \`[table]\`\n\n`);
  const owner = t.owners?.[0]?.displayName ?? t.owners?.[0]?.name ?? "—";
  const tagsList = (t.tags ?? [])
    .map((tag) => tag.tagFQN)
    .filter(Boolean)
    .join(", ");
  md.appendMarkdown(`*Owner:* ${owner}${tagsList ? ` · *Tags:* ${tagsList}` : ""}\n\n`);
  if (t.description) {
    md.appendMarkdown(`**Description:** ${truncate(t.description, 280)}\n\n`);
  }
  if (t.columns && t.columns.length > 0) {
    const cols = t.columns
      .slice(0, 8)
      .map((c) => `${c.name} (${c.dataType ?? "?"})`)
      .join(", ");
    md.appendMarkdown(`**Columns:** ${cols}${t.columns.length > 8 ? ", …" : ""}\n\n`);
  }
  const lineageCmd = vscode.Uri.parse(
    `command:metatree.openLineageForFqn?${encodeURIComponent(JSON.stringify(t.fullyQualifiedName))}`,
  );
  md.appendMarkdown(
    `🔗 [Open in OpenMetadata](${browserUrl}) · [Show Lineage](${lineageCmd})`,
  );
  return md;
}

function truncate(s: string, n: number): string {
  return s.length > n ? `${s.slice(0, n - 1)}…` : s;
}
