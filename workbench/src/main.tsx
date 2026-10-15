import { Component, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import './styles/globals.css';
class WorkspaceBoundary extends Component<{ children: ReactNode }, { error?: string }> {
  state: { error?: string } = {};
  static getDerivedStateFromError(error: Error) { return { error: error.message }; }
  render() { return this.state.error ? <div role="alert" className="p-4 text-sm text-destructive">工作台加载失败：{this.state.error}</div> : this.props.children; }
}
const root = createRoot(document.getElementById('root')!);
import('./app/App').then(({ App }) => root.render(<WorkspaceBoundary><App /></WorkspaceBoundary>)).catch(error => root.render(<div role="alert">工作台加载失败：{String(error)}</div>));
