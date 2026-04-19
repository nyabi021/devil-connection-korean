<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onMount, tick } from 'svelte';
  import Modal from './lib/Modal.svelte';
  import {
    autoDetectGamePath,
    cancelInstall,
    pickGameFolder,
    startInstall,
    validateGamePath
  } from './lib/tauri';
  import type { InstallEvent, LogEntry, LogLevel } from './lib/types';

  const CREDITS =
    "메인 시나리오 번역 검수: Ewan  ·  이미지 번역: 토니, 체퓨  ·  영상 번역: 민버드";
  const FOOTER =
    '본 프로그램은 ㈜넥슨코리아 메이플스토리 서체 및 ㈜우아한형제들 배달의민족 꾸불림체를 사용합니다.';

  type ModalState =
    | { kind: 'info' | 'success' | 'warning' | 'error'; title: string; body: string }
    | { kind: 'confirm'; title: string; body: string; action: 'cancel' | 'close' }
    | null;

  let gamePath = $state('');
  let pathValid = $state<boolean | null>(null);
  let detecting = $state(false);
  let installing = $state(false);
  let progress = $state(0);
  let showProgress = $state(false);
  let logs = $state<LogEntry[]>([]);
  let modal = $state<ModalState>(null);
  let logContainer: HTMLDivElement | undefined = $state();
  let cancelRequested = $state(false);
  let closeAfterInstall = $state(false);

  function pushLog(level: LogLevel, message: string) {
    logs.push({ level, message });
    queueScroll();
  }

  async function queueScroll() {
    await tick();
    if (logContainer) logContainer.scrollTop = logContainer.scrollHeight;
  }

  function printWelcome() {
    pushLog('info', '데빌 커넥션 한글패치를 시작합니다.');
    pushLog('info', '');
    pushLog(
      'success',
      "메인 시나리오 번역 검수 'Ewan'님, 이미지 번역 '토니', '체퓨'님, 영상 번역 '민버드'님께 진심으로 감사드립니다."
    );
    pushLog('info', '');
    pushLog('info', "'자동 감지' 버튼을 클릭하거나 게임 경로를 직접 선택해주세요.");
  }

  async function revalidatePath() {
    const trimmed = gamePath.trim();
    if (!trimmed) {
      pathValid = null;
      return;
    }
    try {
      pathValid = await validateGamePath(trimmed);
    } catch {
      pathValid = false;
    }
  }

  let pathDebounce: ReturnType<typeof setTimeout> | undefined;
  function onPathInput() {
    clearTimeout(pathDebounce);
    pathDebounce = setTimeout(revalidatePath, 180);
  }

  async function handleAutoDetect() {
    if (detecting || installing) return;
    detecting = true;
    pushLog('info', '게임 경로를 자동으로 검색 중...');
    try {
      const found = await autoDetectGamePath();
      if (found) {
        gamePath = found;
        pathValid = true;
        pushLog('success', '게임을 찾았습니다!');
        pushLog('info', `경로: ${found}`);
      } else {
        pathValid = null;
        pushLog('warning', '게임 경로를 자동으로 찾지 못했습니다.');
        pushLog('info', "'찾아보기' 버튼으로 직접 선택해주세요.");
        modal = {
          kind: 'warning',
          title: '경로 감지 실패',
          body: "게임 경로를 자동으로 찾지 못했습니다.\n\n'찾아보기' 버튼을 눌러 직접 선택해주세요."
        };
      }
    } catch (err) {
      pushLog('error', `자동 감지 실패: ${err}`);
    } finally {
      detecting = false;
    }
  }

  async function handleBrowse() {
    if (installing) return;
    try {
      const picked = await pickGameFolder();
      if (!picked) return;
      gamePath = picked;
      const ok = await validateGamePath(picked);
      pathValid = ok;
      if (ok) {
        pushLog('success', `게임 경로 선택: ${picked}`);
      } else {
        pushLog('info', `게임 경로 선택: ${picked}`);
        pushLog('warning', 'app.asar 파일을 찾을 수 없습니다. 올바른 게임 폴더인지 확인하세요.');
      }
    } catch (err) {
      pushLog('error', `폴더 선택 실패: ${err}`);
    }
  }

  async function handleInstall() {
    if (installing) return;
    const path = gamePath.trim();
    if (!path) {
      modal = { kind: 'warning', title: '경로 없음', body: '게임 경로를 먼저 선택해주세요.' };
      return;
    }
    const ok = await validateGamePath(path);
    if (!ok) {
      pathValid = false;
      modal = {
        kind: 'warning',
        title: '잘못된 게임 경로',
        body: '선택한 폴더에서 게임 파일(app.asar)을 찾을 수 없습니다.\n\n올바른 게임 설치 폴더를 선택해주세요.'
      };
      return;
    }

    installing = true;
    showProgress = true;
    progress = 0;
    cancelRequested = false;
    closeAfterInstall = false;

    try {
      await startInstall(path, handleInstallEvent);
    } catch (err) {
      installing = false;
      showProgress = false;
      cancelRequested = false;
      closeAfterInstall = false;
      modal = { kind: 'error', title: '설치 오류', body: `설치 시작 중 오류: ${err}` };
    }
  }

  async function requestCancel(closeAfter: boolean) {
    cancelRequested = true;
    closeAfterInstall = closeAfter;
    modal = {
      kind: 'info',
      title: closeAfter ? '종료 준비 중' : '취소 중',
      body: closeAfter
        ? '설치를 취소하고 원본 파일 복원을 시도한 뒤 앱을 종료합니다.\n잠시만 기다려주세요.'
        : '설치를 취소하고 원본 파일을 복원하는 중입니다.\n잠시만 기다려주세요.'
    };
    try {
      await cancelInstall(closeAfter);
    } catch (err) {
      cancelRequested = false;
      closeAfterInstall = false;
      modal = { kind: 'error', title: '취소 오류', body: `설치 취소 중 오류: ${err}` };
    }
  }

  function handleInstallEvent(ev: InstallEvent) {
    if (ev.kind === 'log') {
      pushLog(ev.data.level, ev.data.message);
    } else if (ev.kind === 'progress') {
      progress = ev.data.value;
    } else if (ev.kind === 'finished') {
      installing = false;
      showProgress = false;
      cancelRequested = false;
      if (closeAfterInstall) {
        closeAfterInstall = false;
        return;
      }
      if (ev.data.success) {
        pathValid = null;
        modal = { kind: 'success', title: '설치 완료', body: ev.data.message };
      } else {
        modal = { kind: 'error', title: '설치 오류', body: ev.data.message };
      }
    }
  }

  async function dismissModal() {
    modal = null;
  }

  async function handleConfirmModal() {
    if (!modal || modal.kind !== 'confirm') {
      return;
    }
    await requestCancel(modal.action === 'close');
  }

  onMount(() => {
    printWelcome();
    let unlisten: (() => void) | undefined;

    listen('install-close-requested', () => {
      if (!installing || cancelRequested) {
        return;
      }
      modal = {
        kind: 'confirm',
        title: '설치 중',
        body: '설치가 진행 중입니다. 종료하시겠습니까?\n원본 파일 복원을 시도한 뒤 앱을 종료합니다.',
        action: 'close'
      };
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  });

  let canInstall = $derived(!!gamePath.trim() && !installing && !detecting);
  let pathBorderClass = $derived(
    pathValid === true ? 'valid' : pathValid === false ? 'invalid' : 'neutral'
  );
</script>

<main>
  <header>
    <h1>데빌 커넥션 한글패치</h1>
    <div class="subtitle">でびるコネクショん</div>
    <div class="credits">{CREDITS}</div>
  </header>

  <section class="card path-card">
    <label for="game-path" class="label">게임 경로</label>
    <div class="path-row">
      <div class="input-wrap {pathBorderClass}">
        <input
          id="game-path"
          type="text"
          bind:value={gamePath}
          oninput={onPathInput}
          placeholder="게임이 설치된 경로를 선택하세요"
          disabled={installing}
          spellcheck="false"
          autocomplete="off"
        />
        {#if pathValid === true}
          <span class="status-dot valid" aria-label="유효한 경로">✓</span>
        {:else if pathValid === false}
          <span class="status-dot invalid" aria-label="잘못된 경로">!</span>
        {/if}
      </div>
    </div>
    <div class="btn-row">
      <div class="left-btns">
        <button
          class="btn btn-ghost"
          onclick={handleAutoDetect}
          disabled={detecting || installing}
        >
          {#if detecting}
            <span class="spinner"></span>
            검색 중
          {:else}
            자동 감지
          {/if}
        </button>
        <button class="btn btn-ghost" onclick={handleBrowse} disabled={installing}>
          찾아보기
        </button>
        {#if installing}
          <button class="btn btn-ghost" onclick={() => (modal = {
            kind: 'confirm',
            title: '설치 취소',
            body: '설치를 취소하고 원본 파일을 복원할까요?',
            action: 'cancel'
          })} disabled={cancelRequested}>
            {cancelRequested ? '취소 중...' : '설치 취소'}
          </button>
        {/if}
      </div>
      <button class="btn btn-primary" onclick={handleInstall} disabled={!canInstall}>
        {installing ? '설치 중...' : '설치 시작'}
      </button>
    </div>
  </section>

  {#if showProgress}
    <section class="card progress-card">
      <div class="progress-header">
        <span class="progress-label">진행률</span>
        <span class="progress-value">{progress}%</span>
      </div>
      <div class="progress-track">
        <div class="progress-fill" style="width: {progress}%"></div>
      </div>
    </section>
  {/if}

  <section class="card log-card">
    <div class="log-header">
      <span class="label">설치 로그</span>
      <span class="log-count">{logs.length}</span>
    </div>
    <div class="log-body" bind:this={logContainer}>
      {#each logs as entry, i (i)}
        {#if entry.message === ''}
          <div class="log-spacer"></div>
        {:else}
          <div class="log-line log-{entry.level}">
            <span class="log-glyph">
              {#if entry.level === 'success'}✓
              {:else if entry.level === 'warning'}⚠
              {:else if entry.level === 'error'}✕
              {:else}·{/if}
            </span>
            <span class="log-text">{entry.message}</span>
          </div>
        {/if}
      {/each}
    </div>
  </section>

  <footer>{FOOTER}</footer>
</main>

{#if modal}
  {#if modal.kind === 'confirm'}
    <Modal
      kind="confirm"
      title={modal.title}
      body={modal.body}
      confirmLabel={modal.action === 'close' ? '종료' : '취소'}
      cancelLabel="계속 진행"
      onConfirm={handleConfirmModal}
      onCancel={dismissModal}
    />
  {:else}
    <Modal kind={modal.kind} title={modal.title} body={modal.body} onConfirm={dismissModal} />
  {/if}
{/if}

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 28px 28px 16px;
    height: 100vh;
    overflow: hidden;
  }

  header {
    text-align: center;
    padding-bottom: 2px;
  }
  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--text);
  }
  .subtitle {
    margin-top: 2px;
    font-size: 11.5px;
    color: var(--text-subtle);
    letter-spacing: 0.02em;
  }
  .credits {
    margin-top: 8px;
    font-size: 11px;
    color: var(--text-muted);
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px 18px;
    box-shadow: var(--shadow-sm);
  }

  .label {
    display: block;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    margin-bottom: 10px;
  }

  .input-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface-alt);
    border: 1.5px solid var(--border);
    border-radius: var(--radius-xs);
    padding: 0 12px;
    transition: border-color 140ms ease, background 140ms ease;
  }
  .input-wrap.valid {
    border-color: var(--accent);
    background: var(--path-valid-bg);
  }
  .input-wrap.invalid {
    border-color: var(--path-warn);
    background: var(--path-warn-bg);
  }
  .input-wrap:focus-within {
    border-color: var(--accent);
    background: var(--surface);
  }

  input {
    flex: 1;
    border: none;
    outline: none;
    background: transparent;
    padding: 10px 0;
    font-size: 13px;
    color: var(--text);
    font-family: inherit;
  }
  input::placeholder {
    color: var(--text-subtle);
  }
  input:disabled {
    color: var(--text-muted);
  }

  .status-dot {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 11px;
    font-weight: 700;
    color: white;
  }
  .status-dot.valid {
    background: var(--accent);
  }
  .status-dot.invalid {
    background: var(--path-warn);
  }

  .btn-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-top: 12px;
  }
  .left-btns {
    display: flex;
    gap: 6px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 9px 16px;
    font-size: 12.5px;
    font-weight: 500;
    border-radius: var(--radius-xs);
    border: 1px solid transparent;
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease,
      transform 60ms ease;
    white-space: nowrap;
  }
  .btn:active:not(:disabled) {
    transform: translateY(1px);
  }
  .btn-ghost {
    background: transparent;
    color: var(--text-muted);
    border-color: var(--border);
  }
  .btn-ghost:hover:not(:disabled) {
    background: var(--surface-alt);
    color: var(--text);
    border-color: var(--border-strong);
  }
  .btn-ghost:disabled {
    opacity: 0.5;
  }
  .btn-primary {
    background: var(--accent);
    color: white;
    min-width: 118px;
  }
  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn-primary:disabled {
    background: var(--btn-disabled-bg);
    color: var(--btn-disabled-fg);
  }

  .spinner {
    width: 11px;
    height: 11px;
    border: 1.5px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.75s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .progress-card {
    padding: 14px 18px;
  }
  .progress-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 8px;
  }
  .progress-label {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .progress-value {
    font-size: 13px;
    font-weight: 600;
    color: var(--accent-soft-fg);
    font-variant-numeric: tabular-nums;
  }
  .progress-track {
    position: relative;
    height: 6px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-hover));
    border-radius: 3px;
    transition: width 300ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .log-card {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding-bottom: 14px;
  }
  .log-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 8px;
  }
  .log-header .label {
    margin-bottom: 0;
  }
  .log-count {
    font-size: 11px;
    color: var(--text-subtle);
    font-variant-numeric: tabular-nums;
  }
  .log-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    background: var(--surface-alt);
    border-radius: var(--radius-xs);
    padding: 10px 14px;
    font-family: 'SF Mono', 'Menlo', 'Consolas', 'D2Coding', monospace;
    font-size: 11.5px;
    line-height: 1.6;
    color: var(--text);
  }
  .log-line {
    display: flex;
    gap: 8px;
    padding: 1px 0;
  }
  .log-glyph {
    flex-shrink: 0;
    width: 12px;
    color: var(--text-subtle);
    font-family: inherit;
  }
  .log-text {
    flex: 1;
    word-break: break-all;
    white-space: pre-wrap;
  }
  .log-spacer {
    height: 6px;
  }
  .log-info {
    color: var(--text);
  }
  .log-info .log-glyph {
    color: var(--text-subtle);
  }
  .log-success {
    color: var(--success);
  }
  .log-success .log-glyph {
    color: var(--success);
  }
  .log-warning {
    color: var(--warning);
  }
  .log-warning .log-glyph {
    color: var(--warning);
  }
  .log-error {
    color: var(--error);
  }
  .log-error .log-glyph {
    color: var(--error);
  }

  footer {
    text-align: center;
    font-size: 10.5px;
    color: var(--text-subtle);
    padding-top: 2px;
  }
</style>
