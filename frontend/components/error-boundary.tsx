'use client';

import React, { Component, type ReactNode } from 'react';
import { AlertTriangle, RefreshCcw, Home } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onReset?: () => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: React.ErrorInfo | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null
    };
  }

  static getDerivedStateFromError(error: Error): Partial<ErrorBoundaryState> {
    return {
      hasError: true,
      error
    };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    console.error('[ErrorBoundary] Caught error:', error, errorInfo);
    this.setState({
      error,
      errorInfo
    });
  }

  handleReset = (): void => {
    this.setState({
      hasError: false,
      error: null,
      errorInfo: null
    });
    this.props.onReset?.();
  };

  handleReload = (): void => {
    window.location.reload();
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="min-h-screen flex items-center justify-center p-4 bg-gradient-to-br from-bg via-muted to-bg">
          <div className="max-w-md w-full">
            <div className="rounded-2xl border border-red-900/50 bg-gradient-to-br from-red-950/30 to-card/50 p-8 shadow-2xl backdrop-blur-sm">
              {/* Error Icon */}
              <div className="flex justify-center mb-6">
                <div className="rounded-full bg-red-500/20 p-4 ring-4 ring-red-500/10">
                  <AlertTriangle className="h-12 w-12 text-red-400" />
                </div>
              </div>

              {/* Error Message */}
              <div className="text-center space-y-3 mb-6">
                <h2 className="text-2xl font-bold text-fg">
                  Oops! Something went wrong
                </h2>
                <p className="text-muted-fg">
                  We encountered an unexpected error. Don&apos;t worry, your data is safe.
                </p>
              </div>

              {/* Error Details (in development) */}
              {process.env.NODE_ENV === 'development' && this.state.error && (
                <div className="mb-6 rounded-lg bg-muted/40 p-4 border border-border">
                  <p className="text-sm font-mono text-red-400 break-all">
                    {this.state.error.toString()}
                  </p>
                  {this.state.errorInfo && (
                    <details className="mt-2">
                      <summary className="text-xs text-muted-fg cursor-pointer hover:text-fg">
                        Stack trace
                      </summary>
                      <pre className="text-xs text-muted-fg mt-2 overflow-auto max-h-40">
                        {this.state.errorInfo.componentStack}
                      </pre>
                    </details>
                  )}
                </div>
              )}

              {/* Action Buttons */}
              <div className="flex flex-col gap-3">
                <Button
                  onClick={this.handleReset}
                  className="w-full bg-gradient-to-r from-red-500 to-red-600 hover:from-red-600 hover:to-red-700 text-white font-bold"
                >
                  <RefreshCcw className="h-4 w-4 mr-2" />
                  Try Again
                </Button>
                <Button
                  onClick={this.handleReload}
                  variant="secondary"
                  className="w-full"
                >
                  <Home className="h-4 w-4 mr-2" />
                  Reload Page
                </Button>
              </div>

              {/* Support Info */}
              <div className="mt-6 pt-6 border-t border-border text-center">
                <p className="text-sm text-muted-fg">
                  If this problem persists, please{' '}
                  <a
                    href="https://github.com/JRudyRay/NearSplitter/issues"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-red-400 hover:text-red-300 underline"
                  >
                    report the issue
                  </a>
                </p>
              </div>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

// Hook version for functional components
export function useErrorHandler() {
  const [error, setError] = React.useState<Error | null>(null);

  const resetError = React.useCallback(() => {
    setError(null);
  }, []);

  const handleError = React.useCallback((error: Error) => {
    console.error('[useErrorHandler]', error);
    setError(error);
  }, []);

  if (error) {
    throw error;
  }

  return { handleError, resetError };
}
