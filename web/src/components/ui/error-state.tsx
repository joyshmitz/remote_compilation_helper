'use client';

import { useState } from 'react';
import { AlertTriangle, RefreshCw, ChevronDown, ChevronUp } from 'lucide-react';
import { Button } from './button';
import { cn } from '@/lib/utils';

interface ErrorStateProps {
  /** The error to display. Can be an Error object or a string. */
  error: Error | string;
  /** Callback function to retry the failed operation. */
  onRetry?: () => void;
  /** Custom title for the error state. Defaults to "Something went wrong". */
  title?: string;
  /** Helpful hint to show users about how to resolve the error. */
  hint?: string;
  /** Additional CSS classes for the container. */
  className?: string;
  /** Whether the retry button should show a loading state. */
  isRetrying?: boolean;
}

function getErrorMessage(error: Error | string): string {
  if (typeof error === 'string') return error;
  return error.message || 'An unexpected error occurred';
}

function getErrorStack(error: Error | string): string | undefined {
  if (typeof error === 'string') return undefined;
  return error.stack;
}

/**
 * A reusable error state component with retry functionality.
 *
 * Features:
 * - Displays error message with icon
 * - Optional helpful hints for common errors
 * - Retry button to re-fetch data
 * - Collapsible technical details for debugging
 */
export function ErrorState({
  error,
  onRetry,
  title = 'Something went wrong',
  hint,
  className,
  isRetrying = false,
}: ErrorStateProps) {
  const [showDetails, setShowDetails] = useState(false);
  const errorMessage = getErrorMessage(error);
  const errorStack = getErrorStack(error);

  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center p-8 text-center',
        className
      )}
      data-testid="error-state"
      role="alert"
      aria-live="polite"
    >
      <div className="flex items-center justify-center w-16 h-16 rounded-full bg-destructive/10 mb-4">
        <AlertTriangle className="h-8 w-8 text-destructive" />
      </div>

      <h3 className="font-semibold text-lg">{title}</h3>

      <p className="text-muted-foreground mt-2 max-w-md">{errorMessage}</p>

      {hint && (
        <p className="text-sm text-muted-foreground mt-3 max-w-md bg-muted/50 px-4 py-2 rounded-md">
          {hint}
        </p>
      )}

      <div className="flex items-center gap-3 mt-6">
        {onRetry && (
          <Button
            onClick={onRetry}
            disabled={isRetrying}
            className="min-w-[120px]"
          >
            <RefreshCw
              className={cn('mr-2 h-4 w-4', isRetrying && 'animate-spin')}
            />
            {isRetrying ? 'Retrying...' : 'Try Again'}
          </Button>
        )}

        {errorStack && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowDetails(!showDetails)}
            aria-expanded={showDetails}
            aria-controls="error-details"
          >
            {showDetails ? (
              <>
                <ChevronUp className="mr-1 h-4 w-4" />
                Hide Details
              </>
            ) : (
              <>
                <ChevronDown className="mr-1 h-4 w-4" />
                Show Details
              </>
            )}
          </Button>
        )}
      </div>

      {showDetails && errorStack && (
        <pre
          id="error-details"
          className="mt-4 p-4 bg-muted rounded-md text-xs text-left overflow-auto max-w-full max-h-48 w-full"
        >
          <code>{errorStack}</code>
        </pre>
      )}
    </div>
  );
}

/** Common error hints for RCH-specific errors */
export const errorHints = {
  daemonConnection: 'Make sure rchd is running: rchd start',
  networkError: 'Check your network connection and try again.',
  timeout: 'The request took too long. The server might be busy.',
  workerUnavailable: 'No workers are available. Check worker status.',
  notFound: 'The requested resource was not found.',
};
