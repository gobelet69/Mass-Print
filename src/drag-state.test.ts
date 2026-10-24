import { describe, expect, it } from "vitest";
import { destinationForDrop, nextExternalDragState } from "./drag-state";

describe("external drag state", () => {
  it("activates only after a supported Finder entry", () => {
    expect(nextExternalDragState(false, { type: "enter", paths: ["/tmp/one.pdf"] }, false)).toBe(true);
    expect(nextExternalDragState(false, { type: "enter", paths: ["/tmp/folder"] }, false)).toBe(false);
  });

  it("does not reactivate from a trailing over event after drop", () => {
    expect(nextExternalDragState(false, { type: "over" }, false)).toBe(false);
    expect(nextExternalDragState(true, { type: "drop", paths: ["/tmp/one.pdf"] }, false)).toBe(false);
  });

  it("always resets on leave and ignores internal row dragging", () => {
    expect(nextExternalDragState(true, { type: "leave" }, false)).toBe(false);
    expect(nextExternalDragState(true, { type: "over" }, true)).toBe(false);
  });
});

describe("internal insertion destinations", () => {
  it("moves the last row to the beginning", () => {
    expect(destinationForDrop(3, 0, "before", 4)).toBe(0);
  });

  it("moves the first row to the end", () => {
    expect(destinationForDrop(0, 3, "after", 4)).toBe(3);
  });

  it("accounts for removing the source before insertion", () => {
    expect(destinationForDrop(0, 2, "before", 4)).toBe(1);
    expect(destinationForDrop(3, 1, "after", 4)).toBe(2);
  });
});
