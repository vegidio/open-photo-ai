import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Button, Paper, Typography } from '@mui/material';
import { AnalyticsEvent, track } from '@/analytics';
import { getErrorMessage } from '@/utils/errors.ts';

type ErrorBoundaryProps = {
    children: ReactNode;
};

type ErrorBoundaryState = {
    error: Error | null;
};

/**
 * Catches render errors anywhere below it so an uncaught exception degrades to a recoverable screen instead of a blank
 * window. This is a class component because error boundaries have no hook equivalent — `componentDidCatch` and
 * `getDerivedStateFromError` are the only APIs React exposes for it.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
    state: ErrorBoundaryState = { error: null };

    static getDerivedStateFromError(error: Error): ErrorBoundaryState {
        return { error };
    }

    componentDidCatch(error: Error, info: ErrorInfo) {
        // The component stack is the useful half of a render crash, and it exists only here — it is not on the Error.
        console.error('Unhandled render error', error, info.componentStack);
        track(AnalyticsEvent.RenderCrashed, { reason: getErrorMessage(error) });
    }

    render() {
        const { error } = this.state;
        if (!error) return this.props.children;

        return (
            <div className='flex h-screen items-center justify-center bg-[#353535] p-8'>
                <Paper elevation={8} className='flex max-w-lg flex-col gap-4 p-6'>
                    <Typography variant='h6'>Something went wrong</Typography>

                    <Typography variant='body2' color='text.secondary'>
                        Open Photo AI hit an unexpected error and can't continue. Reloading usually fixes it; your
                        settings are kept, but any unsaved work in the current session is lost.
                    </Typography>

                    <Typography variant='caption' color='text.secondary' className='break-words font-mono'>
                        {getErrorMessage(error)}
                    </Typography>

                    <Button variant='contained' onClick={() => window.location.reload()} className='self-start'>
                        Reload
                    </Button>
                </Paper>
            </div>
        );
    }
}
