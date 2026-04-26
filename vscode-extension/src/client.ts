// Thin OpenMetadata REST client used by every MetaTree feature.
// Maps 401/404/429/5xx to actionable strings instead of throwing raw errors.

export interface OmConfig {
  host: string;
  token: string;
}

export interface SearchHit {
  id: string;
  name: string;
  fullyQualifiedName: string;
  entityType: string;
  score: number;
  source: Record<string, unknown>;
}

export class OpenMetadataError extends Error {
  constructor(public readonly status: number, message: string) {
    super(message);
    this.name = "OpenMetadataError";
  }
}

export class OpenMetadataClient {
  private host: string;
  private token: string;

  constructor(config: OmConfig) {
    this.host = config.host.replace(/\/+$/, "");
    this.token = config.token;
  }

  isConfigured(): boolean {
    return !!this.host && !!this.token;
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T | null> {
    const url = path.startsWith("http") ? path : `${this.host}${path}`;
    let lastErr: unknown;
    for (let attempt = 0; attempt < 3; attempt++) {
      let response: Response;
      try {
        response = await fetch(url, {
          ...init,
          headers: {
            Authorization: `Bearer ${this.token}`,
            Accept: "application/json",
            "Content-Type": "application/json",
            ...(init.headers ?? {}),
          },
        });
      } catch (err) {
        lastErr = err;
        await sleep(2 ** attempt * 250);
        continue;
      }
      if (response.status === 404) {
        return null;
      }
      if (response.status === 401) {
        throw new OpenMetadataError(
          401,
          `401 Unauthorized for ${url}. Check your MetaTree JWT (Settings → Bots → ingestion-bot).`,
        );
      }
      if (response.status === 429) {
        const retryAfter = Number(response.headers.get("retry-after")) || 2 ** attempt;
        await sleep(retryAfter * 1000);
        continue;
      }
      if (response.status >= 500) {
        if (attempt < 2) {
          await sleep(2 ** attempt * 500);
          continue;
        }
        throw new OpenMetadataError(
          response.status,
          `${response.status} from ${url}. Check the OpenMetadata host URL.`,
        );
      }
      if (!response.ok) {
        const body = await safeText(response);
        throw new OpenMetadataError(response.status, `${response.status} from ${url}: ${body}`);
      }
      const text = await response.text();
      if (!text) return null;
      return JSON.parse(text) as T;
    }
    throw new OpenMetadataError(0, `Could not reach ${url}: ${String(lastErr)}`);
  }

  async search(
    query: string,
    index = "table_search_index",
    limit = 5,
  ): Promise<SearchHit[]> {
    const qs = new URLSearchParams({ q: query, index, limit: String(limit) });
    const data = await this.request<{ hits?: { hits?: RawHit[] } }>(
      `/v1/search/query?${qs.toString()}`,
    );
    const hits = data?.hits?.hits ?? [];
    return hits.map(toSearchHit);
  }

  async tableByFqn(fqn: string, fields = "owners,tags,columns"): Promise<TableEntity | null> {
    const path = `/v1/tables/name/${encodeURIComponent(fqn)}?fields=${fields}`;
    return this.request<TableEntity>(path);
  }

  async lineageById(
    id: string,
    upstreamDepth = 2,
    downstreamDepth = 2,
  ): Promise<LineagePayload | null> {
    const qs = new URLSearchParams({
      upstreamDepth: String(upstreamDepth),
      downstreamDepth: String(downstreamDepth),
    });
    return this.request<LineagePayload>(`/v1/lineage/table/${id}?${qs.toString()}`);
  }

  async qualityForFqn(fqn: string): Promise<QualityPayload | null> {
    const qs = new URLSearchParams({
      entityLink: `<#E::table::${fqn}>`,
      fields: "tests,testCaseResults",
    });
    return this.request<QualityPayload>(`/v1/dataQuality/testSuites?${qs.toString()}`);
  }

  /** Build a deep link to the entity in the OpenMetadata UI. */
  entityUrl(fqn: string, kind = "table"): string {
    const uiBase = this.host.replace(/\/api\/?$/, "");
    return `${uiBase}/${kind}/${encodeURIComponent(fqn)}`;
  }
}

interface RawHit {
  _id?: string;
  _score?: number;
  _source?: Record<string, unknown>;
}

export interface TableEntity {
  id: string;
  name: string;
  fullyQualifiedName: string;
  description?: string;
  owners?: { name?: string; displayName?: string }[];
  tags?: { tagFQN: string }[];
  columns?: {
    name: string;
    dataType?: string;
    description?: string;
    tags?: { tagFQN: string }[];
  }[];
}

export interface LineageNode {
  id: string;
  name?: string;
  fullyQualifiedName?: string;
  type?: string;
  description?: string;
  owners?: { name?: string; displayName?: string }[];
}

export interface LineageEdge {
  fromEntity: string | { id?: string };
  toEntity: string | { id?: string };
}

export interface LineagePayload {
  nodes?: LineageNode[];
  upstreamEdges?: LineageEdge[];
  downstreamEdges?: LineageEdge[];
}

export interface QualityTest {
  name: string;
  testCaseStatus?: string;
  status?: string;
  failureReason?: string;
  message?: string;
}

export interface QualityPayload {
  data?: { tests?: QualityTest[] }[];
}

function toSearchHit(hit: RawHit): SearchHit {
  const src = hit._source ?? {};
  return {
    id: (hit._id as string) ?? (src["id"] as string) ?? "",
    name: (src["name"] as string) ?? (src["displayName"] as string) ?? "",
    fullyQualifiedName:
      (src["fullyQualifiedName"] as string) ?? (src["name"] as string) ?? "",
    entityType: (src["entityType"] as string) ?? (src["type"] as string) ?? "table",
    score: (hit._score as number) ?? 0,
    source: src,
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function safeText(response: Response): Promise<string> {
  try {
    return (await response.text()).slice(0, 200);
  } catch {
    return "";
  }
}
