import {installExceptionHandlers, reportException} from './shared/api/diagnostics';
import {WorkspaceBoundary} from './shared/ui/WorkspaceBoundary';
import { createRoot } from 'react-dom/client';
import './styles/globals.css';
installExceptionHandlers();
const root = createRoot(document.getElementById('root')!);
import('./app/App').then(({ App }) => root.render(<WorkspaceBoundary><App /></WorkspaceBoundary>)).catch(error => {reportException('entry_import',error);root.render(<div role="alert">工作台加载失败：{String(error)}</div>);});
