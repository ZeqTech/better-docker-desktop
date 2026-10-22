import { describe, expect, it } from "vitest";
import { EMPTY_FORM, lines, parsePresets, toOptions, upsertPreset, validateForm, type RunForm } from "./runform";

const form = (p: Partial<RunForm>): RunForm => ({ ...EMPTY_FORM, ...p });

describe("toOptions", () => {
  it("splits multi-line fields and drops blanks", () => {
    const o = toOptions("nginx", form({ name: " web ", ports: "8080:80\n\n 9090:90 \r\n", env: "A=1", volumes: "d:/d", command: " nginx -g x " }));
    expect(o).toEqual({ image: "nginx", name: "web", ports: ["8080:80", "9090:90"], env: ["A=1"], volumes: ["d:/d"], restart: "", command: "nginx -g x" });
    expect(lines("")).toEqual([]);
  });
});

describe("validateForm", () => {
  it("accepts a good form", () => {
    expect(validateForm(form({ name: "web-1", ports: "8080:80\n5353:53/udp", env: "A=1\nB_2=x y", volumes: "data:/d:ro\nC:\\src:/app", command: 'sh -c "echo hi"' }))).toEqual([]);
  });
  it("reports each problem", () => {
    const errs = validateForm(form({ name: "-bad", ports: "80\n99999:80", env: "nokey", volumes: "data\nx:rel", command: 'echo "oops' }));
    expect(errs).toHaveLength(7);
    expect(errs.join("\n")).toMatch(/Name/);
    expect(errs.join("\n")).toMatch(/1-65535/);
    expect(errs.join("\n")).toMatch(/unclosed/);
  });
});

describe("presets", () => {
  it("ignores malformed storage", () => {
    expect(parsePresets(null)).toEqual([]);
    expect(parsePresets("{oops")).toEqual([]);
    expect(parsePresets('{"a":1}')).toEqual([]);
    expect(parsePresets('[{"name":"","form":{}},{"x":1},{"name":"ok","form":{"ports":"1:1","restart":"weird"}}]')).toEqual([
      { name: "ok", form: { ...EMPTY_FORM, ports: "1:1" } },
    ]);
  });
  it("de-duplicates by name", () => {
    const raw = JSON.stringify([{ name: "a", form: { ports: "1:1" } }, { name: "a", form: { ports: "2:2" } }]);
    expect(parsePresets(raw)).toHaveLength(1);
  });
  it("upserts and keeps the list sorted", () => {
    let list = upsertPreset([], { name: "zeta", form: form({ ports: "1:1" }) });
    list = upsertPreset(list, { name: "alpha", form: EMPTY_FORM });
    list = upsertPreset(list, { name: "zeta", form: form({ ports: "2:2" }) });
    expect(list.map((p) => p.name)).toEqual(["alpha", "zeta"]);
    expect(list[1].form.ports).toBe("2:2");
  });
});
