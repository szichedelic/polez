import { Component, type ReactNode, type ErrorInfo } from 'react';

interface Props {
  section: string;
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`[${this.props.section}] Component error:`, error);
    console.error('Component stack:', info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="bg-zinc-900 border border-red-800/50 rounded p-4">
          <div className="flex items-center justify-between">
            <div>
              <span className="text-red-400 text-sm font-medium">{this.props.section} crashed</span>
              <p className="text-zinc-500 text-xs mt-1">
                {this.state.error?.message || 'An unexpected error occurred'}
              </p>
            </div>
            <button
              onClick={() => this.setState({ hasError: false, error: null })}
              className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-1 rounded text-xs"
            >
              Retry
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
