import { invoke, Channel } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { InstallEvent } from './types';

export async function autoDetectGamePath(): Promise<string | null> {
  return (await invoke<string | null>('auto_detect_game_path')) ?? null;
}

export async function validateGamePath(path: string): Promise<boolean> {
  return await invoke<boolean>('validate_game_path', { path });
}

export async function pickGameFolder(): Promise<string | null> {
  const result = await open({
    directory: true,
    multiple: false,
    title: 'Devil Connection 게임 폴더를 선택하세요'
  });
  return typeof result === 'string' ? result : null;
}

export async function startInstall(
  gamePath: string,
  onEvent: (ev: InstallEvent) => void
): Promise<void> {
  const channel = new Channel<InstallEvent>();
  channel.onmessage = onEvent;
  await invoke('start_install', { gamePath, onEvent: channel });
}

export async function cancelInstall(closeAfter = false): Promise<void> {
  await invoke('cancel_install', { closeAfter });
}
