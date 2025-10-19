import React from 'react';
import ReactDOM from 'react-dom/client';
import { ThemeInit } from '../.flowbite-react/init.tsx';
import { Home } from './Home.tsx';
import './style.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
        <ThemeInit />
        <Home />
    </React.StrictMode>,
);
