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
import { ErrorBoundary } from '@/features/shared/ErrorBoundary';
// Side-effect import: initialises i18next from the persisted language. Biome sorts this among the other path
// imports, which is fine — ESM evaluates every import before this module's body, so i18n is always ready before
// createRoot().render() runs. The rule it imposes on the rest of the app is that t() is never called at module
// scope, since that would capture the language at evaluation time and never update.
import '@/i18n';
import { useSettingsStore } from '@/stores';

// Initialize analytics before the app mounts, honoring the user's persisted opt-out, then record the app open.
initAnalytics(useSettingsStore.getState().analyticsEnabled);
track(AnalyticsEvent.AppOpen);

// Keep the analytics collection flag mirrored to the persisted opt-out — the store is the single source of truth, so any
// change (including a Settings-cancel revert via restoreSnapshot) re-syncs analytics without call-site coordination.
useSettingsStore.subscribe((state, prev) => {
    if (state.analyticsEnabled !== prev.analyticsEnabled) setAnalyticsEnabled(state.analyticsEnabled);
});

// Dark only, deliberately. The UI's colours are hardcoded dark hex literals throughout (bg-[#212121], #009aff and so
// on), so a light MUI palette would not produce a working light theme - it would produce dark panels with light
// controls. The light theme and the prefers-color-scheme check that used to sit here were both unreachable; making
// light mode real means moving those literals into theme tokens first.
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
    return (
        <ThemeProvider theme={darkTheme}>
            <StyledEngineProvider enableCssLayer>
                <GlobalStyles styles='@layer theme, base, mui, components, utilities;' />

                <SnackbarProvider
                    anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
                    TransitionComponent={Grow}
                >
                    <ErrorBoundary>
                        <App />
                    </ErrorBoundary>
                </SnackbarProvider>
            </StyledEngineProvider>
        </ThemeProvider>
    );
};

const root = document.getElementById('root');
if (!root) throw new Error('index.html is missing the #root element the app mounts into');

ReactDOM.createRoot(root).render(<Main />);
