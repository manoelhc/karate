import { createRoot } from 'react-dom/client';
import { App, newClient } from './App';
import './i18n';
import './style.css';
const element = document.getElementById('root');
if (!element) throw new Error('Missing application root');
createRoot(element).render(<App client={newClient()} />);
