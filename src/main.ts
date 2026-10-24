import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import {
  containsSupportedPath,
  destinationForDrop,
  nextExternalDragState,
  type ExternalDragEvent,
} from "./drag-state";
import type {
  AddReport,
  BatchPrintReport,
  DocumentEntry,
  FinderActionStatus,
  PrinterInfo,
  PrintSettings,
  QueueSnapshot,
  SortCriterion,
  SortDirection,
} from "./types";
import "./styles.css";

const app = document.querySelector<HTMLDivElement>("#app")!;

let snapshot: QueueSnapshot = {
  documents: [],
  totalKnownPages: 0,
  unknownPageCount: 0,
  activeSort: { criterion: "original", direction: "ascending" },
};
let printers: PrinterInfo[] = [];
let selectedIds = new Set<string>();
let draggedId: string | null = null;
let internalDragActive = false;
let busy = false;
let dropActive = false;
let ignoreExternalEventsUntil = 0;
let finderActionStatus: FinderActionStatus = {
  installed: false,
  valid: false,
  needsUpdate: false,
  path: "",
  message: "The Finder Quick Action is not installed",
};
let notice: { kind: "info" | "error" | "success"; message: string } | null = null;

const settings: PrintSettings = {
  printer: "",
  copies: 1,
  grayscale: false,
  duplex: false,
  paperSize: null,
  landscape: false,
  fitToPage: true,
};

const sortLabels: Record<SortCriterion, string> = {
  original: "Original Order",
  name: "Name",
  modified: "Modified Date",
  size: "File Size",
  pages: "Page Count",
  fileType: "File Type",
};

function escapeHtml(value: string): string {
  return value.replace(
    /[&<>'"]/g,
    (character) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[
        character
      ]!,
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function documentSubtitle(document: DocumentEntry): string {
  const pages = document.pageCount
    ? `${document.pageCount} ${document.pageCount === 1 ? "page" : "pages"}`
    : "Page count unavailable";
  return `${escapeHtml(document.extension)} · ${pages} · ${formatBytes(document.sizeBytes)}`;
}

function renderDocument(document: DocumentEntry, index: number): string {
  const selected = selectedIds.has(document.id);
  const warning = document.status === "needsConversion";
  const statusLabel: Record<DocumentEntry["status"], string> = {
    ready: "Ready",
    needsConversion: "Conversion",
    preparing: "Preparing",
    submitted: "Submitted",
    failed: "Failed",
    canceled: "Canceled",
  };
  return `
    <li class="document-row${selected ? " selected" : ""}" data-id="${document.id}"
        tabindex="0" role="option" aria-selected="${selected}">
      <button class="drag-handle" draggable="true" aria-label="Drag ${escapeHtml(document.name)}" title="Drag to reorder">⠿</button>
      <div class="file-icon ${document.extension.toLowerCase()}" aria-hidden="true">${escapeHtml(document.extension.slice(0, 4))}</div>
      <div class="document-copy">
        <div class="document-name" title="${escapeHtml(document.path)}">${escapeHtml(document.name)}</div>
        <div class="document-meta">${documentSubtitle(document)}</div>
        ${warning ? `<div class="document-warning">Requires LibreOffice conversion</div>` : ""}
      </div>
      <div class="status-pill ${document.status}" title="${escapeHtml(document.statusMessage ?? "Ready to print")}">
        ${statusLabel[document.status]}
      </div>
      <div class="row-actions">
        <button class="icon-button move-up" aria-label="Move ${escapeHtml(document.name)} up" ${index === 0 ? "disabled" : ""}>↑</button>
        <button class="icon-button move-down" aria-label="Move ${escapeHtml(document.name)} down" ${index === snapshot.documents.length - 1 ? "disabled" : ""}>↓</button>
        <button class="icon-button remove" aria-label="Remove ${escapeHtml(document.name)}">×</button>
      </div>
    </li>`;
}

function sortDirectionDescription(criterion: SortCriterion, direction: SortDirection): string {
  const ascending = direction === "ascending";
  switch (criterion) {
    case "name": return ascending ? "A to Z" : "Z to A";
    case "modified": return ascending ? "Oldest first" : "Newest first";
    case "size": return ascending ? "Smallest first" : "Largest first";
    case "pages": return ascending ? "Fewest first" : "Most first";
    case "fileType": return ascending ? "Type A to Z" : "Type Z to A";
    case "original": return ascending ? "First added first" : "Last added first";
  }
}

function renderSortToolbar(): string {
  const active = snapshot.activeSort;
  const criterion = active?.criterion ?? "original";
  const direction = active?.direction ?? "ascending";
  return `
    <div class="queue-sortbar">
      <div class="queue-heading">
        <strong>${snapshot.documents.length} ${snapshot.documents.length === 1 ? "Document" : "Documents"}</strong>
        <span>${active ? sortDirectionDescription(criterion, direction) : "Custom Order"}</span>
      </div>
      <div class="sort-controls">
        <label for="sort-criterion">Sort by</label>
        <select id="sort-criterion" aria-label="Sort documents">
          ${active ? "" : `<option value="custom" selected>Custom Order</option>`}
          ${Object.entries(sortLabels).map(([value, label]) => `<option value="${value}" ${active?.criterion === value ? "selected" : ""}>${label}</option>`).join("")}
        </select>
        <button id="sort-direction" class="direction-button" ${active ? "" : "disabled"}
          aria-label="${active ? `Reverse sort. Current direction: ${sortDirectionDescription(criterion, direction)}` : "Direction unavailable for custom order"}"
          title="${active ? sortDirectionDescription(criterion, direction) : "Custom order"}">
          ${direction === "ascending" ? "↑" : "↓"}
        </button>
      </div>
    </div>`;
}

function render(): void {
  const hasDocuments = snapshot.documents.length > 0;
  const pageSummary = snapshot.unknownPageCount
    ? `${snapshot.totalKnownPages} known pages · ${snapshot.unknownPageCount} unknown`
    : `${snapshot.totalKnownPages} ${snapshot.totalKnownPages === 1 ? "page" : "pages"}`;

  app.innerHTML = `
    <main class="app-shell${dropActive ? " drop-active" : ""}">
      <header class="toolbar">
        <div class="brand">
          <div class="brand-mark" aria-hidden="true">M</div>
          <div><h1>Mass Print</h1><p>Batch printing, in your order.</p></div>
        </div>
        <div class="toolbar-actions">
          <button id="finder-button" class="secondary" ${finderActionStatus.valid && !finderActionStatus.needsUpdate || busy ? "disabled" : ""}
            title="${escapeHtml(finderActionStatus.message)}">
            ${finderActionStatus.installed ? finderActionStatus.needsUpdate || !finderActionStatus.valid ? "Repair Finder Action" : "Finder Action ✓" : "Install Finder Action"}
          </button>
          ${finderActionStatus.installed && finderActionStatus.valid ? `<button id="remove-finder-button" class="secondary compact" title="Remove Finder Quick Action">Remove</button>` : ""}
          <button id="clear-button" class="secondary" ${!hasDocuments || busy ? "disabled" : ""}>Clear Queue</button>
          <button id="add-button" class="primary" ${busy ? "disabled" : ""}>＋ Add Files</button>
        </div>
      </header>

      ${notice ? `<div class="notice ${notice.kind}" role="status"><span>${escapeHtml(notice.message)}</span><button id="dismiss-notice" aria-label="Dismiss">×</button></div>` : ""}

      <section class="queue-panel" aria-label="Print queue">
        ${
          hasDocuments
            ? `${renderSortToolbar()}
              <ol class="document-list" role="listbox" aria-multiselectable="true">
                ${snapshot.documents.map(renderDocument).join("")}
              </ol>`
            : `<div class="empty-state">
                <div class="drop-illustration" aria-hidden="true"><span>↓</span><div>PDF</div></div>
                <h2>Drop files here to print</h2>
                <p>Drag documents from Finder or choose files to get started.</p>
                <button id="empty-add-button" class="primary">Choose Files</button>
                <p class="format-hint">PDF, PNG, JPEG, TIFF, TXT · Office formats with LibreOffice</p>
              </div>`
        }
        <div class="drop-overlay" aria-hidden="true"><div>Drop files to add them to the queue</div></div>
      </section>

      <footer class="print-panel">
        <div class="summary">
          <strong>${snapshot.documents.length} ${snapshot.documents.length === 1 ? "document" : "documents"}</strong>
          <span>${hasDocuments ? pageSummary : "Queue is empty"}</span>
        </div>
        <div class="controls">
          <label class="field printer-field"><span>Printer</span>
            <select id="printer" ${busy ? "disabled" : ""}>
              ${printers.length === 0 ? `<option value="">No printer detected</option>` : printers.map((printer) => `<option value="${escapeHtml(printer.name)}" ${settings.printer === printer.name ? "selected" : ""}>${escapeHtml(printer.name)}${printer.isDefault ? " — Default" : ""}</option>`).join("")}
            </select>
          </label>
          <label class="field copies-field"><span>Copies</span>
            <input id="copies" type="number" min="1" max="999" value="${settings.copies}" ${busy ? "disabled" : ""} />
          </label>
          <label class="check-field"><input id="grayscale" type="checkbox" ${settings.grayscale ? "checked" : ""} ${busy ? "disabled" : ""}/><span>Grayscale</span></label>
          <label class="check-field"><input id="duplex" type="checkbox" ${settings.duplex ? "checked" : ""} ${busy ? "disabled" : ""}/><span>Two-sided</span></label>
        </div>
        <div class="print-actions">
          ${busy ? `<button id="cancel-button" class="danger">Cancel</button>` : ""}
          <button id="print-button" class="print-button" ${!hasDocuments || !settings.printer || busy ? "disabled" : ""}>
            ${busy ? `<span class="spinner"></span> Submitting…` : "Print All"}
          </button>
        </div>
      </footer>
    </main>`;

  bindEvents();
}

function bindEvents(): void {
  document.querySelector("#add-button")?.addEventListener("click", chooseFiles);
  document.querySelector("#empty-add-button")?.addEventListener("click", chooseFiles);
  document.querySelector("#clear-button")?.addEventListener("click", clearQueue);
  document.querySelector("#print-button")?.addEventListener("click", printAll);
  document.querySelector("#finder-button")?.addEventListener("click", installFinderAction);
  document.querySelector("#remove-finder-button")?.addEventListener("click", uninstallFinderAction);
  document.querySelector("#cancel-button")?.addEventListener("click", cancelPrinting);
  document.querySelector("#dismiss-notice")?.addEventListener("click", () => {
    notice = null;
    render();
  });

  const sortCriterion = document.querySelector<HTMLSelectElement>("#sort-criterion");
  sortCriterion?.addEventListener("change", async () => {
    if (sortCriterion.value !== "custom") {
      await sortQueue(sortCriterion.value as SortCriterion, snapshot.activeSort?.direction ?? "ascending");
    }
  });
  document.querySelector("#sort-direction")?.addEventListener("click", async () => {
    const active = snapshot.activeSort;
    if (active) {
      await sortQueue(active.criterion, active.direction === "ascending" ? "descending" : "ascending");
    }
  });

  const printer = document.querySelector<HTMLSelectElement>("#printer");
  printer?.addEventListener("change", () => {
    settings.printer = printer.value;
    render();
  });
  const copies = document.querySelector<HTMLInputElement>("#copies");
  copies?.addEventListener("change", () => {
    settings.copies = Math.min(999, Math.max(1, Number(copies.value) || 1));
    render();
  });
  const grayscale = document.querySelector<HTMLInputElement>("#grayscale");
  grayscale?.addEventListener("change", () => (settings.grayscale = grayscale.checked));
  const duplex = document.querySelector<HTMLInputElement>("#duplex");
  duplex?.addEventListener("change", () => (settings.duplex = duplex.checked));

  document.querySelectorAll<HTMLElement>(".document-row").forEach((row) => {
    const id = row.dataset.id!;
    const handle = row.querySelector<HTMLElement>(".drag-handle");
    row.addEventListener("click", (event) => selectRow(id, event));
    handle?.addEventListener("dragstart", (event) => startInternalDrag(event as DragEvent, id, row));
    handle?.addEventListener("dragend", () => endInternalDrag());
    row.addEventListener("dragover", (event) => updateInsertionIndicator(event as DragEvent, row));
    row.addEventListener("dragleave", (event) => {
      const related = (event as DragEvent).relatedTarget as Node | null;
      if (!related || !row.contains(related)) clearInsertionIndicator(row);
    });
    row.addEventListener("drop", async (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (!draggedId || draggedId === id) {
        endInternalDrag();
        return;
      }
      const sourceIndex = snapshot.documents.findIndex((item) => item.id === draggedId);
      const targetIndex = snapshot.documents.findIndex((item) => item.id === id);
      const placement = row.dataset.dropPlacement === "after" ? "after" : "before";
      const destination = destinationForDrop(sourceIndex, targetIndex, placement, snapshot.documents.length);
      const movedId = draggedId;
      endInternalDrag();
      await reorder(movedId, destination);
    });
    row.querySelector(".remove")?.addEventListener("click", async (event) => {
      event.stopPropagation();
      await removeDocument(id);
    });
    row.querySelector(".move-up")?.addEventListener("click", async (event) => {
      event.stopPropagation();
      await moveDocument(id, -1);
    });
    row.querySelector(".move-down")?.addEventListener("click", async (event) => {
      event.stopPropagation();
      await moveDocument(id, 1);
    });
  });
}

function startInternalDrag(event: DragEvent, id: string, row: HTMLElement): void {
  internalDragActive = true;
  draggedId = id;
  setDropActive(false);
  event.dataTransfer?.setData("text/x-mass-print-document", id);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  row.classList.add("dragging");
}

function updateInsertionIndicator(event: DragEvent, row: HTMLElement): void {
  if (!internalDragActive || !draggedId) return;
  event.preventDefault();
  event.stopPropagation();
  if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  document.querySelectorAll(".document-row.drop-before, .document-row.drop-after")
    .forEach((item) => clearInsertionIndicator(item as HTMLElement));
  const bounds = row.getBoundingClientRect();
  const placement = event.clientY < bounds.top + bounds.height / 2 ? "before" : "after";
  row.dataset.dropPlacement = placement;
  row.classList.add(placement === "before" ? "drop-before" : "drop-after");
}

function clearInsertionIndicator(row: HTMLElement): void {
  row.classList.remove("drop-before", "drop-after");
  delete row.dataset.dropPlacement;
}

function endInternalDrag(): void {
  internalDragActive = false;
  draggedId = null;
  document.querySelectorAll<HTMLElement>(".document-row").forEach((row) => {
    row.classList.remove("dragging");
    clearInsertionIndicator(row);
  });
  setDropActive(false);
}

function selectRow(id: string, event: Event): void {
  const mouseEvent = event as MouseEvent;
  if (mouseEvent.metaKey || mouseEvent.ctrlKey) {
    selectedIds.has(id) ? selectedIds.delete(id) : selectedIds.add(id);
  } else {
    selectedIds = new Set([id]);
  }
  render();
}

function setDropActive(active: boolean): void {
  if (internalDragActive && active) active = false;
  if (dropActive === active) return;
  dropActive = active;
  document.querySelector(".app-shell")?.classList.toggle("drop-active", active);
  document.querySelector(".queue-panel")?.classList.toggle("external-drag", active);
}

function resetDragState(): void {
  ignoreExternalEventsUntil = performance.now() + 250;
  endInternalDrag();
  setDropActive(false);
}

async function refreshQueue(): Promise<void> {
  snapshot = await invoke<QueueSnapshot>("queue_snapshot");
  selectedIds = new Set([...selectedIds].filter((id) => snapshot.documents.some((item) => item.id === id)));
  render();
}

async function addPaths(paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  try {
    const report = await invoke<AddReport>("add_files", { paths });
    if (report.rejected.length) {
      const first = report.rejected[0];
      notice = {
        kind: report.added.length ? "info" : "error",
        message: `${report.rejected.length} file(s) not added. ${first.reason}`,
      };
    } else {
      notice = { kind: "success", message: `${report.added.length} file(s) added to the queue.` };
    }
    await refreshQueue();
  } catch (error) {
    notice = { kind: "error", message: `Files could not be added: ${String(error)}` };
    render();
  }
}

async function chooseFiles(): Promise<void> {
  const selected = await open({
    multiple: true,
    directory: false,
    title: "Add documents to Mass Print",
    filters: [
      { name: "Printable Documents", extensions: ["pdf", "png", "jpg", "jpeg", "tif", "tiff", "txt", "rtf", "doc", "docx", "odt", "html", "htm", "xls", "xlsx", "ppt", "pptx", "ods", "odp"] },
    ],
  });
  if (selected) await addPaths(Array.isArray(selected) ? selected : [selected]);
}

async function removeDocument(id: string): Promise<void> {
  await invoke("remove_document", { id });
  await refreshQueue();
}

async function clearQueue(): Promise<void> {
  await invoke("clear_queue");
  selectedIds.clear();
  notice = null;
  resetDragState();
  await refreshQueue();
}

async function reorder(id: string, destination: number): Promise<void> {
  await invoke("reorder_document", { id, destination });
  await refreshQueue();
}

async function moveDocument(id: string, delta: number): Promise<void> {
  await invoke("move_document", { id, delta });
  await refreshQueue();
}

async function sortQueue(criterion: SortCriterion, direction: SortDirection): Promise<void> {
  await invoke("sort_queue", { criterion, direction });
  await refreshQueue();
}

async function printAll(): Promise<void> {
  if (!settings.printer || snapshot.documents.length === 0) return;
  busy = true;
  notice = null;
  resetDragState();
  render();
  try {
    const report = await invoke<BatchPrintReport>("print_all", { settings });
    await refreshQueue();
    showPrintResult(report);
  } catch (error) {
    notice = { kind: "error", message: String(error) };
  } finally {
    busy = false;
    render();
  }
}

function showPrintResult(report: BatchPrintReport): void {
  const submitted = report.jobs.filter((job) => job.outcome === "submitted").length;
  const failed = report.jobs.filter((job) => job.outcome === "failed").length;
  const canceled = report.jobs.filter((job) => job.outcome === "canceled").length;
  notice = failed
    ? { kind: "error", message: `${submitted} submitted, ${failed} failed${canceled ? `, ${canceled} canceled` : ""}. Open the affected row for details.` }
    : { kind: canceled ? "info" : "success", message: `${submitted} print job(s) accepted by the spooler${canceled ? `; ${canceled} not submitted` : ""}.` };
}

async function cancelPrinting(): Promise<void> {
  await invoke("cancel_printing");
  notice = { kind: "info", message: "Cancellation requested. No additional documents will be submitted." };
}

async function loadPrinters(): Promise<void> {
  try {
    printers = await invoke<PrinterInfo[]>("list_printers");
    settings.printer = printers.find((printer) => printer.isDefault)?.name ?? printers[0]?.name ?? "";
  } catch (error) {
    notice = { kind: "error", message: `Printers are unavailable: ${String(error)}` };
  }
}

async function loadFinderActionStatus(): Promise<void> {
  try {
    finderActionStatus = await invoke<FinderActionStatus>("finder_action_status");
  } catch {
    finderActionStatus = {
      installed: false,
      valid: false,
      needsUpdate: false,
      path: "",
      message: "Finder Quick Action status is unavailable",
    };
  }
}

async function installFinderAction(): Promise<void> {
  try {
    await invoke("install_finder_action");
    await loadFinderActionStatus();
    notice = {
      kind: "success",
      message: "Finder action installed and Finder refreshed. If it is hidden, enable it in System Settings → General → Login Items & Extensions → Finder Extensions.",
    };
  } catch (error) {
    notice = { kind: "error", message: String(error) };
  }
  render();
}

async function uninstallFinderAction(): Promise<void> {
  try {
    await invoke("uninstall_finder_action");
    await loadFinderActionStatus();
    notice = { kind: "success", message: "The Mass Print Finder action was removed." };
  } catch (error) {
    notice = { kind: "error", message: String(error) };
  }
  render();
}

document.addEventListener("keydown", async (event) => {
  if (event.key === "Escape") {
    resetDragState();
  } else if (event.metaKey && event.key.toLowerCase() === "o") {
    event.preventDefault();
    await chooseFiles();
  } else if (event.metaKey && event.key.toLowerCase() === "p") {
    event.preventDefault();
    await printAll();
  } else if (event.metaKey && event.key.toLowerCase() === "a" && snapshot.documents.length) {
    event.preventDefault();
    selectedIds = new Set(snapshot.documents.map((item) => item.id));
    render();
  } else if ((event.key === "Backspace" || event.key === "Delete") && selectedIds.size) {
    event.preventDefault();
    for (const id of selectedIds) await invoke("remove_document", { id });
    await refreshQueue();
  }
});

window.addEventListener("blur", resetDragState);
document.addEventListener("visibilitychange", () => {
  if (document.hidden) resetDragState();
});
document.addEventListener("dragend", resetDragState);

getCurrentWebview().onDragDropEvent(async (event) => {
  const payload = event.payload as ExternalDragEvent;
  if (performance.now() < ignoreExternalEventsUntil && (payload.type === "enter" || payload.type === "over")) {
    setDropActive(false);
    return;
  }
  setDropActive(nextExternalDragState(dropActive, payload, internalDragActive));
  if (payload.type === "drop") {
    ignoreExternalEventsUntil = performance.now() + 250;
    setDropActive(false);
    try {
      if (containsSupportedPath(payload.paths)) await addPaths(payload.paths);
    } finally {
      setDropActive(false);
    }
  }
});

void listen("queue-changed", () => {
  resetDragState();
  void refreshQueue();
});

async function initialize(): Promise<void> {
  render();
  await Promise.all([refreshQueue(), loadPrinters(), loadFinderActionStatus()]);
  render();
}

initialize().catch((error) => {
  notice = { kind: "error", message: `Mass Print could not start: ${String(error)}` };
  render();
});
