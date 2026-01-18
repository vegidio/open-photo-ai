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

const lightTheme = createTheme({
    palette: {
        mode: 'light',
    },
});

const darkTheme = createTheme({
    palette: {
        mode: 'dark',
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
