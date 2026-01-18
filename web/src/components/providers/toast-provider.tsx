'use client';

import { useTheme } from 'next-themes';
import { Toaster } from 'sonner';

export function ToastProvider() {
  const { resolvedTheme } = useTheme();
  const theme = resolvedTheme === 'dark'
    ? 'dark'
    : resolvedTheme === 'light'
      ? 'light'
      : 'system';

  return (
    <Toaster
      theme={theme}
      position="bottom-right"
      richColors
      closeButton
      duration={4000}
      toastOptions={{
        className:
          'rounded-xl border border-border bg-surface text-foreground shadow-lg',
        descriptionClassName: 'text-muted-foreground',
      }}
    />
  );
}
