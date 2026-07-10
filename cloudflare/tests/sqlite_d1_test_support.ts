import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import type { ElyD1PreparedStatement, ElyD1Result } from "../src/bindings.js";
import type { RecordedD1Database } from "./devices_test_support.js";

export class SqliteD1Database implements RecordedD1Database {
  readonly authBinds: unknown[][] = [];
  readonly authQueries: string[] = [];
  readonly batches: number[] = [];
  readonly binds: unknown[][] = [];
  readonly queries: string[] = [];
  readonly sessionConstraints: string[] = [];
  private beforeBatchSql: string | undefined;

  constructor(
    private readonly databasePath: string,
    beforeBatchSql?: string,
  ) {
    this.beforeBatchSql = beforeBatchSql;
  }

  prepare(sql: string): ElyD1PreparedStatement {
    this.queries.push(sql);
    return new SqliteD1Statement(this, sql);
  }

  async batch<T>(statements: ElyD1PreparedStatement[]): Promise<T[]> {
    this.batches.push(statements.length);
    if (this.beforeBatchSql !== undefined) {
      execute(this.databasePath, this.beforeBatchSql);
      this.beforeBatchSql = undefined;
    }
    const prepared = statements.map((statement) => {
      assert.ok(statement instanceof SqliteD1Statement);
      return statement.sql();
    });
    const script = [
      ".bail on",
      "PRAGMA foreign_keys = ON;",
      "BEGIN IMMEDIATE;",
      ...prepared.flatMap((sql, index) => [
        `.print __ELY_BEGIN_${index}`,
        sql,
        `.print __ELY_CHANGES_${index}`,
        "SELECT changes() AS __ely_changes;",
        `.print __ELY_END_${index}`,
      ]),
      "COMMIT;",
    ].join("\n");
    const output = sqlite(this.databasePath, script, true);
    const lines = output.trim().split(/\r?\n/).filter(Boolean);
    const results = prepared.map((_, index) => {
      const begin = lines.indexOf(`__ELY_BEGIN_${index}`);
      const changesMarker = lines.indexOf(`__ELY_CHANGES_${index}`);
      const end = lines.indexOf(`__ELY_END_${index}`);
      assert.ok(begin >= 0 && changesMarker > begin && end > changesMarker);
      const rows = lines
        .slice(begin + 1, changesMarker)
        .flatMap((line) => JSON.parse(line) as unknown[]);
      const changeRows = JSON.parse(lines[changesMarker + 1] ?? "[]") as {
        __ely_changes?: unknown;
      }[];
      const value = changeRows[0]?.__ely_changes;
      assert.equal(typeof value, "number");
      return { results: rows, meta: { changes: value } };
    });
    return results as T[];
  }

  async exec(sql: string): Promise<unknown> {
    execute(this.databasePath, sql);
    return {};
  }

  withSession(constraint: "first-primary"): SqliteD1Database {
    this.sessionConstraints.push(constraint);
    return this;
  }

  rows<T>(sql: string): T[] {
    return query(this.databasePath, sql) as T[];
  }
}

class SqliteD1Statement implements ElyD1PreparedStatement {
  private values: unknown[] = [];

  constructor(
    private readonly database: SqliteD1Database,
    private readonly queryText: string,
  ) {}

  bind(...values: unknown[]): ElyD1PreparedStatement {
    this.values = values;
    this.database.binds.push(values);
    return this;
  }

  async first<T>(): Promise<T | null> {
    return this.database.rows<T>(this.sql())[0] ?? null;
  }

  async all<T>(): Promise<ElyD1Result<T>> {
    return { results: this.database.rows<T>(this.sql()) };
  }

  async run(): Promise<unknown> {
    const rows = this.database.rows<{ changes: number }>(
      `${this.sql()}\nSELECT changes() AS changes;`,
    );
    return { results: [], meta: { changes: rows[0]?.changes ?? 0 } };
  }

  sql(): string {
    let index = 0;
    const sql = this.queryText.replace(/\?/g, () => sqlLiteral(this.values[index++]));
    assert.equal(index, this.values.length, "D1 bind count must match SQL placeholders");
    return `${sql.trim().replace(/;$/, "")};`;
  }
}

export function execute(databasePath: string, sql: string): void {
  sqlite(databasePath, `.bail on\nPRAGMA foreign_keys = ON;\n${sql}`);
}

export function query(databasePath: string, sql: string): Record<string, unknown>[] {
  const output = sqlite(databasePath, `PRAGMA foreign_keys = ON;\n${sql}`, true);
  return output.trim() === "" ? [] : JSON.parse(output) as Record<string, unknown>[];
}

function sqlite(databasePath: string, sql: string, json = false): string {
  try {
    return execFileSync("sqlite3", [...(json ? ["-json"] : []), databasePath], {
      input: sql,
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
  } catch (error) {
    const stderr = typeof error === "object" && error !== null && "stderr" in error
      ? String(error.stderr)
      : "";
    throw new Error(`${error instanceof Error ? error.message : String(error)}\n${stderr}`);
  }
}

function sqlLiteral(value: unknown): string {
  if (value === null) return "NULL";
  if (typeof value === "string") return `'${value.replaceAll("'", "''")}'`;
  if (typeof value === "number" && Number.isFinite(value)) return value.toString();
  throw new TypeError("Unsupported SQLite test binding");
}
