import {Component,type ReactNode,type ErrorInfo} from 'react';
import {reportException} from '../api/diagnostics';
/** 保留原有错误界面，同时将渲染异常交给统一诊断。 */
export class WorkspaceBoundary extends Component<{children:ReactNode},{error?:string}>{
 state:{error?:string}={};
 static getDerivedStateFromError(error:Error){return {error:error.message};}
 componentDidCatch(error:Error,info:ErrorInfo){reportException('react_boundary',error,`${error.stack??''}\n${info.componentStack??''}`);}
 render(){return this.state.error?<div role="alert" className="p-4 text-sm text-destructive">工作台加载失败：{this.state.error}</div>:this.props.children;}
}
