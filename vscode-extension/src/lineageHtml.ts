// Pure HTML rendering for the lineage webview. Kept free of `vscode` imports
// so the unit test runner can require this file directly under plain Node.

export interface VisGraphForRender {
  nodes: { id: string; label: string; group: string; title: string }[];
  edges: { from: string; to: string; arrows: string }[];
}

export function renderHtml(
  graph: VisGraphForRender,
  rootId: string,
  fqn: string,
): string {
  const data = JSON.stringify(graph);
  const escapedFqn = escapeHtml(fqn);
  const escapedRoot = JSON.stringify(rootId);
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
<title>MetaTree Lineage</title>
<style>
  html, body, #network { height: 100%; width: 100%; margin: 0; padding: 0; background: var(--vscode-editor-background); color: var(--vscode-editor-foreground); }
  #toolbar { padding: 8px 12px; font-family: var(--vscode-font-family); border-bottom: 1px solid var(--vscode-panel-border); }
  #network { height: calc(100% - 36px); }
</style>
<script src="https://cdnjs.cloudflare.com/ajax/libs/vis-network/9.1.9/vis-network.min.js"></script>
</head>
<body>
  <div id="toolbar"><strong>${escapedFqn}</strong> · double-click a node to drill in</div>
  <div id="network"></div>
  <script>
    const vscode = acquireVsCodeApi();
    const data = ${data};
    const rootId = ${escapedRoot};
    const groupColors = {
      table:     { background: '#3b82f6', border: '#1e40af' },
      dashboard: { background: '#f97316', border: '#c2410c' },
      pipeline:  { background: '#10b981', border: '#047857' },
      topic:     { background: '#a855f7', border: '#6b21a8' },
      current:   { background: '#facc15', border: '#a16207' },
    };
    const styledNodes = data.nodes.map(n => ({
      ...n,
      shape: 'box',
      color: groupColors[n.group] || groupColors.table,
      font: { color: '#ffffff' },
    }));
    const network = new vis.Network(
      document.getElementById('network'),
      { nodes: new vis.DataSet(styledNodes), edges: new vis.DataSet(data.edges) },
      {
        layout: { hierarchical: { direction: 'LR', sortMethod: 'directed', levelSeparation: 180 } },
        physics: false,
        interaction: { hover: true, tooltipDelay: 200 },
      }
    );
    network.on('doubleClick', params => {
      if (params.nodes.length === 0) return;
      const id = params.nodes[0];
      if (id === rootId) return;
      const node = styledNodes.find(n => n.id === id);
      if (node) vscode.postMessage({ type: 'openEntity', fqn: node.label });
    });
  </script>
</body>
</html>`;
}

export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
