import { describe, expect, it } from "vitest";
import { CrashDetector, affected, parseEvent, type DockerEvent } from "./events";

const line = (o: object) => JSON.stringify(o);

describe("parseEvent", () => {
  it("reads a container die event", () => {
    const ev = parseEvent(line({ Type: "container", Action: "die", Actor: { ID: "abc", Attributes: { name: "web", exitCode: "137" } } }))!;
    expect(ev).toMatchObject({ type: "container", action: "die", id: "abc", name: "web", exitCode: "137" });
  });
  it("drops the detail after a colon", () => {
    expect(parseEvent(line({ Type: "container", Action: "health_status: unhealthy", Actor: { ID: "x" } }))!.action).toBe("health_status");
  });
  it("returns null for junk and tolerates unknown types", () => {
    expect(parseEvent("nope")).toBeNull();
    expect(parseEvent(line({ Type: "network", Action: "create" }))!.type).toBe("");
  });
});

describe("affected", () => {
  it("routes events to listings", () => {
    const ev = (type: DockerEvent["type"]): DockerEvent => ({ type, action: "x", id: "", name: "", exitCode: "", signal: "" });
    expect(affected(ev("container"))).toEqual(["containers"]);
    expect(affected(ev("image"))).toEqual(["images"]);
    expect(affected(ev("volume"))).toEqual(["volumes"]);
    expect(affected(ev(""))).toEqual([]);
  });
});

describe("CrashDetector", () => {
  const ev = (action: string, exitCode = "", id = "c1"): DockerEvent => ({ type: "container", action, id, name: "web", exitCode, signal: "" });

  it("flags a die with a non-zero exit code", () => {
    const d = new CrashDetector();
    expect(d.feed(ev("die", "1"))).toEqual({ name: "web", reason: "exited with code 1" });
  });
  it("ignores clean exits", () => {
    expect(new CrashDetector().feed(ev("die", "0"))).toBeNull();
  });
  it("ignores a die that follows a kill (docker stop)", () => {
    const d = new CrashDetector();
    d.feed(ev("kill"), 1000);
    expect(d.feed(ev("die", "137"), 2000)).toBeNull();
  });
  it("flags a die long after an old kill", () => {
    const d = new CrashDetector(5000);
    d.feed(ev("kill"), 0);
    expect(d.feed(ev("die", "2"), 60_000)).not.toBeNull();
  });
  it("tracks containers independently", () => {
    const d = new CrashDetector();
    d.feed(ev("kill", "", "a"), 0);
    expect(d.feed(ev("die", "1", "b"), 10)).not.toBeNull();
  });
  it("flags out-of-memory kills", () => {
    expect(new CrashDetector().feed(ev("oom"))).toEqual({ name: "web", reason: "ran out of memory" });
  });
});
