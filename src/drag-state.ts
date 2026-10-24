const SUPPORTED_EXTENSIONS = new Set([
  "pdf", "png", "jpg", "jpeg", "tif", "tiff", "txt", "rtf", "doc", "docx",
  "odt", "html", "htm", "xls", "xlsx", "ppt", "pptx", "ods", "odp",
]);

export type ExternalDragEvent =
  | { type: "enter"; paths: string[] }
  | { type: "over" }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };

export function containsSupportedPath(paths: string[]): boolean {
  return paths.some((path) => {
    const filename = path.split(/[\\/]/).pop() ?? "";
    const extension = filename.includes(".") ? filename.split(".").pop()?.toLowerCase() : "";
    return extension !== undefined && SUPPORTED_EXTENSIONS.has(extension);
  });
}

export function nextExternalDragState(
  current: boolean,
  event: ExternalDragEvent,
  internalDragActive: boolean,
): boolean {
  if (internalDragActive) return false;
  switch (event.type) {
    case "enter":
      return containsSupportedPath(event.paths);
    case "over":
      // An `over` event has no paths and may arrive after `drop`. It can keep
      // a validated session alive, but it must never create a new one.
      return current;
    case "drop":
    case "leave":
      return false;
  }
}

export function destinationForDrop(
  sourceIndex: number,
  targetIndex: number,
  placement: "before" | "after",
  length: number,
): number {
  if (length <= 1) return 0;
  let destination: number;
  if (placement === "before") {
    destination = sourceIndex < targetIndex ? targetIndex - 1 : targetIndex;
  } else {
    destination = sourceIndex < targetIndex ? targetIndex : targetIndex + 1;
  }
  return Math.max(0, Math.min(length - 1, destination));
}

