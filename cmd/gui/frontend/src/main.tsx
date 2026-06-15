import '@fontsource/roboto/300.css';
import '@fontsource/roboto/400.css';
import '@fontsource/roboto/500.css';
import '@fontsource/roboto/700.css';

import { createTheme, Grow, StyledEngineProvider, ThemeProvider } from '@mui/material';
import GlobalStyles from '@mui/material/GlobalStyles';
import ReactDOM from 'react-dom/client';
import { App } from './App.tsx';
import './style.css';
import { SnackbarProvider } from 'notistack';
import { AnalyticsEvent, initAnalytics, setAnalyticsEnabled, track } from '@/analytics';
import { useSettingsStore } from '@/stores';

// Initialize analytics before the app mounts, honoring the user's persisted opt-out, then record the app open.
initAnalytics(useSettingsStore.getState().analyticsEnabled);
track(AnalyticsEvent.AppOpen);

// Keep the analytics collection flag mirrored to the persisted opt-out — the store is the single source of truth, so any
// change (including a Settings-cancel revert via restoreSnapshot) re-syncs analytics without call-site coordination.
useSettingsStore.subscribe((state, prev) => {
    if (state.analyticsEnabled !== prev.analyticsEnabled) setAnalyticsEnabled(state.analyticsEnabled);
});

const lightTheme = createTheme({
    palette: {
        mode: 'light',
    },
});

const darkTheme = createTheme({
    palette: {
        mode: 'dark',
    },
    components: {
        MuiBackdrop: {
            styleOverrides: {
                // Only darken backdrops that belong to a Dialog (modals). Popovers, menus, and selects render their
                // backdrop under a different root, so they stay untouched.
                root: {
                    '.MuiDialog-root &': {
                        backgroundColor: 'rgba(0, 0, 0, 0.7)',
                    },
                },
            },
        },
    },
});

const Main = () => {
    const isDarkMode = true; // useMediaQuery('(prefers-color-scheme: dark)');

    return (
        <ThemeProvider theme={isDarkMode ? darkTheme : lightTheme}>
            <StyledEngineProvider enableCssLayer>
                <GlobalStyles styles='@layer theme, base, mui, components, utilities;' />

                <SnackbarProvider
                    anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
                    TransitionComponent={Grow}
                >
                    <App />
                </SnackbarProvider>
            </StyledEngineProvider>
        </ThemeProvider>
    );
};

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(<Main />);
