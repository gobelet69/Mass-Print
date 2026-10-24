export type DocumentStatus =
  | "ready"
  | "needsConversion"
  | "preparing"
  | "submitted"
  | "failed"
  | "canceled";

export interface DocumentEntry {
  id: string;
  path: string;
  name: string;
  extension: string;
  sizeBytes: number;
  pageCount: number | null;
  format: string;
  status: DocumentStatus;
  statusMessage: string | null;
}

export interface QueueSnapshot {
  documents: DocumentEntry[];
  totalKnownPages: number;
  unknownPageCount: number;
}

export interface AddFailure {
  path: string;
  reason: string;
}

export interface AddReport {
  added: DocumentEntry[];
  rejected: AddFailure[];
}

export interface PrinterInfo {
  name: string;
  isDefault: boolean;
}

export interface PrintSettings {
  printer: string;
  copies: number;
  grayscale: boolean;
  duplex: boolean;
  paperSize: string | null;
  landscape: boolean;
  fitToPage: boolean;
}

export interface JobReport {
  documentId: string;
  outcome: "submitted" | "failed" | "canceled";
  spoolerJobId: string | null;
  message: string;
}

export interface BatchPrintReport {
  jobs: JobReport[];
}

