import React from 'react';
import ReactDOM from 'react-dom/client';
import { Home } from './Home.tsx';
import './style.css';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
        <Home />
    </React.StrictMode>,
);
