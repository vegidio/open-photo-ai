import '@fontsource/roboto/300.css';
import '@fontsource/roboto/400.css';
import '@fontsource/roboto/500.css';
import '@fontsource/roboto/700.css';

import { createTheme, StyledEngineProvider, ThemeProvider, useMediaQuery } from '@mui/material';
import GlobalStyles from '@mui/material/GlobalStyles';
import ReactDOM from 'react-dom/client';
import { Home } from './Home.tsx';
import './style.css';

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
    const isDarkMode = useMediaQuery('(prefers-color-scheme: dark)');

    return (
        <ThemeProvider theme={isDarkMode ? darkTheme : lightTheme}>
            <StyledEngineProvider enableCssLayer>
                <GlobalStyles styles="@layer theme, base, mui, components, utilities;" />
                <Home />
            </StyledEngineProvider>
        </ThemeProvider>
    );
};

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(<Main />);
