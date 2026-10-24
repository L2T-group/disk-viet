import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './styles/index.css';
import { App } from './app/App';
import { useLangSetting } from './features/settings/lang';

const Root = () => <App key={useLangSetting((s) => s.epoch)} />;

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Root />
  </StrictMode>,
);
