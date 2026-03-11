import { useEffect, useCallback } from 'react';

export interface ShortcutActions {
  onTogglePlay?: () => void;
  onUpload?: () => void;
  onDetect?: () => void;
  onClean?: () => void;
  onSave?: () => void;
  onSeekBack?: () => void;
  onSeekForward?: () => void;
  onShowHelp?: () => void;
}

export function useKeyboardShortcuts(actions: ShortcutActions) {
  const handler = useCallback((e: KeyboardEvent) => {
    const target = e.target as HTMLElement;
    const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT';

    if (e.key === '?' && !isInput) {
      e.preventDefault();
      actions.onShowHelp?.();
      return;
    }

    if (e.key === ' ' && !isInput) {
      e.preventDefault();
      actions.onTogglePlay?.();
      return;
    }

    if (e.key === 'ArrowLeft' && !isInput) {
      e.preventDefault();
      actions.onSeekBack?.();
      return;
    }

    if (e.key === 'ArrowRight' && !isInput) {
      e.preventDefault();
      actions.onSeekForward?.();
      return;
    }

    if (e.key.toLowerCase() === 'd' && !isInput && !(e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      actions.onDetect?.();
      return;
    }

    if ((e.ctrlKey || e.metaKey) && !e.shiftKey) {
      switch (e.key.toLowerCase()) {
        case 'u':
          e.preventDefault();
          actions.onUpload?.();
          break;
        case 'enter':
          e.preventDefault();
          actions.onClean?.();
          break;
        case 's':
          e.preventDefault();
          actions.onSave?.();
          break;
      }
    }
  }, [actions]);

  useEffect(() => {
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handler]);
}

export const SHORTCUT_LIST = [
  { keys: 'Space', description: 'Play / Pause audio' },
  { keys: '\u2190 / \u2192', description: 'Seek audio \u00B15 seconds' },
  { keys: 'Ctrl+U', description: 'Upload file' },
  { keys: 'D', description: 'Run detection' },
  { keys: 'Ctrl+Enter', description: 'Start cleaning' },
  { keys: 'Ctrl+S', description: 'Save cleaned file' },
  { keys: '?', description: 'Show keyboard shortcuts' },
];
