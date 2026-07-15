import { describe, it, expect } from "vitest";
import express from "express";
import request from "supertest";
import { CarnotStore } from "../services/carnotStore.js";
import { mountCarnotCycles } from "./carnot-cycles.js";

describe("GET /api/v1/carnot/snapshot", () => {
  it("returns empty snapshot initially", async () => {
    const app = express();
    const store = new CarnotStore();
    mountCarnotCycles(app, { store, logger: { warn: () => {} } });
    const res = await request(app).get("/api/v1/carnot/snapshot");
    expect(res.status).toBe(200);
    expect(res.body.ok).toBe(true);
    expect(res.body.data.cycles).toEqual([]);
  });
});
