<script lang="ts">
  interface Props {
    kind: 'info' | 'success' | 'warning' | 'error' | 'confirm';
    title: string;
    body: string;
    confirmLabel?: string;
    cancelLabel?: string;
    onConfirm: () => void;
    onCancel?: () => void;
  }

  let {
    kind,
    title,
    body,
    confirmLabel = '확인',
    cancelLabel = '아니오',
    onConfirm,
    onCancel
  }: Props = $props();

  const iconByKind: Record<Props['kind'], string> = {
    info: 'ℹ',
    success: '✓',
    warning: '!',
    error: '✕',
    confirm: '?'
  };

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget && onCancel) onCancel();
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && onCancel) onCancel();
    if (e.key === 'Enter') onConfirm();
  }
</script>

<svelte:window onkeydown={handleKey} />

<div
  class="backdrop"
  role="presentation"
  onclick={handleBackdrop}
  onkeydown={null as unknown as () => void}
>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="modal-title">
    <div class="icon icon-{kind}" aria-hidden="true">{iconByKind[kind]}</div>
    <h2 id="modal-title">{title}</h2>
    <p>{body}</p>
    <div class="actions">
      {#if kind === 'confirm'}
        <button class="btn-secondary" onclick={() => onCancel?.()}>{cancelLabel}</button>
        <button class="btn-primary" onclick={onConfirm}>{confirmLabel}</button>
      {:else}
        <button class="btn-primary" onclick={onConfirm}>{confirmLabel}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--modal-backdrop);
    backdrop-filter: blur(4px);
    display: grid;
    place-items: center;
    z-index: 100;
    animation: fade-in 160ms ease;
  }

  .dialog {
    background: var(--surface);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    padding: 28px 28px 20px;
    width: min(420px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow: auto;
    animation: pop-in 180ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }

  .icon {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 22px;
    font-weight: 700;
    margin-bottom: 14px;
  }
  .icon-info {
    background: var(--icon-info-bg);
    color: var(--icon-info-fg);
  }
  .icon-success {
    background: var(--accent-soft);
    color: var(--accent-soft-fg);
  }
  .icon-warning {
    background: var(--icon-warning-bg);
    color: var(--icon-warning-fg);
  }
  .icon-error {
    background: var(--icon-error-bg);
    color: var(--icon-error-fg);
  }
  .icon-confirm {
    background: var(--icon-warning-bg);
    color: var(--icon-warning-fg);
  }

  h2 {
    margin: 0 0 8px;
    font-size: 17px;
    font-weight: 600;
    color: var(--text);
    letter-spacing: -0.01em;
  }

  p {
    margin: 0 0 20px;
    font-size: 13.5px;
    color: var(--text-muted);
    white-space: pre-wrap;
    line-height: 1.6;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-primary,
  .btn-secondary {
    padding: 9px 18px;
    border-radius: var(--radius-xs);
    border: none;
    font-size: 13px;
    font-weight: 500;
    transition: background 120ms ease, transform 80ms ease;
  }
  .btn-primary {
    background: var(--accent);
    color: white;
  }
  .btn-primary:hover {
    background: var(--accent-hover);
  }
  .btn-primary:active {
    transform: translateY(1px);
  }
  .btn-secondary {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .btn-secondary:hover {
    background: var(--surface-alt);
    color: var(--text);
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  @keyframes pop-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(6px);
    }
  }
</style>
