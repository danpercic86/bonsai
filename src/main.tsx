import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { ErrorBoundary } from './components/ErrorBoundary';
import { startStability } from './obs/stability/install';
import { MockProfiler } from './obs/stability/MockProfiler';
import './styles.css';

const rootEl = document.getElementById('root');
if (!rootEl) {
  throw new Error('missing #root element');
}

// P120: stability detector lifecycle (mock mode registers the lab counters).
startStability();

const app = (
  <ErrorBoundary label="Bonsai">
    <App />
  </ErrorBoundary>
);

ReactDOM.createRoot(rootEl).render(
  <React.StrictMode>
    {/* P120: the commit counter exists only in the mock harness (zero prod cost). */}
    {import.meta.env.VITE_MOCK_IPC === '1' ? <MockProfiler>{app}</MockProfiler> : app}
  </React.StrictMode>,
);
