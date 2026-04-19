export type LogLevel = 'info' | 'success' | 'warning' | 'error';

export type InstallEvent =
  | { kind: 'log'; data: { level: LogLevel; message: string } }
  | { kind: 'progress'; data: { value: number } }
  | { kind: 'finished'; data: { success: boolean; message: string } };

export interface LogEntry {
  level: LogLevel;
  message: string;
}
