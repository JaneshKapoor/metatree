// Helper used by the webview to convert a LineagePayload into the
// nodes/edges shape vis-network wants. Kept as a pure function so it can be
// unit-tested without spinning up VS Code.

import { LineageEdge, LineageNode, LineagePayload } from "../client";

export interface VisNode {
  id: string;
  label: string;
  group: "table" | "dashboard" | "pipeline" | "topic" | "current";
  title: string;
}

export interface VisEdge {
  from: string;
  to: string;
  arrows: "to";
}

export function toVisGraph(
  payload: LineagePayload,
  rootId: string,
): { nodes: VisNode[]; edges: VisEdge[] } {
  const nodes: VisNode[] = [];
  const seen = new Set<string>();
  for (const n of payload.nodes ?? []) {
    if (!n.id || seen.has(n.id)) continue;
    seen.add(n.id);
    nodes.push(toNode(n, rootId));
  }
  if (!seen.has(rootId)) {
    nodes.push({ id: rootId, label: rootId, group: "current", title: rootId });
  }
  const edges: VisEdge[] = [];
  for (const e of payload.upstreamEdges ?? []) edges.push(edgeFor(e));
  for (const e of payload.downstreamEdges ?? []) edges.push(edgeFor(e));
  return { nodes, edges };
}

function toNode(n: LineageNode, rootId: string): VisNode {
  const kind = (n.type ?? "table").toLowerCase();
  const group: VisNode["group"] =
    n.id === rootId
      ? "current"
      : kind === "dashboard"
      ? "dashboard"
      : kind === "pipeline"
      ? "pipeline"
      : kind === "topic"
      ? "topic"
      : "table";
  const label = n.fullyQualifiedName ?? n.name ?? n.id;
  const owner = n.owners?.[0]?.displayName ?? n.owners?.[0]?.name ?? "";
  const title = [label, owner && `Owner: ${owner}`, n.description].filter(Boolean).join("\n");
  return { id: n.id, label, group, title };
}

function edgeFor(edge: LineageEdge): VisEdge {
  return {
    from: idOf(edge.fromEntity),
    to: idOf(edge.toEntity),
    arrows: "to",
  };
}

function idOf(side: LineageEdge["fromEntity"]): string {
  if (typeof side === "string") return side;
  return side?.id ?? "";
}
