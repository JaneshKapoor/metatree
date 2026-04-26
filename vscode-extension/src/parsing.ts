// Pure helpers for token classification. Kept free of `vscode` imports so the
// unit test runner can require this file directly under plain Node.

export const SQL_KEYWORDS = new Set([
  "select", "from", "where", "join", "on", "and", "or", "not", "in", "is",
  "null", "true", "false", "as", "with", "by", "group", "order", "having",
  "limit", "offset", "union", "all", "distinct", "case", "when", "then",
  "else", "end", "left", "right", "inner", "outer", "cross", "full", "using",
  "create", "table", "alter", "drop", "view", "if", "exists", "values",
  "insert", "update", "delete", "set", "into",
]);

export function isLikelyTableReference(word: string): boolean {
  if (!word) return false;
  const last = word.split(".").pop() ?? word;
  if (SQL_KEYWORDS.has(last.toLowerCase())) return false;
  if (last.length < 3) return false;
  if (!/^[A-Za-z_]/.test(last)) return false;
  return true;
}
