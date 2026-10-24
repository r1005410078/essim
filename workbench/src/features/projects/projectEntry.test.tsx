import { expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { ProjectEntry } from './ProjectEntry';
import { WorkbenchTitlebar } from '../command-center/WorkbenchTitlebar';
it('工程检查期间不装配任何工作台业务', () => {
  let mounted=false;
  const html=renderToStaticMarkup(<ProjectEntry>{()=>{mounted=true;throw Error('不应装配工作台');}}</ProjectEntry>);
  expect(mounted).toBe(false);
  expect(html).toContain('正在检查最近工程');
  expect(html).not.toContain('临时会话');
});
it('顶部将历史与检索相邻且三种面板开关都位于右组',()=>{
  const noop=()=>{};
  const html=renderToStaticMarkup(<WorkbenchTitlebar project={<button>当前工程</button>} platform="macos" canBack canForward={false} back={noop} forward={noop} toggle={noop} search={noop} operate={noop}/>);
  expect(html.indexOf('当前工程')).toBeLessThan(html.indexOf('历史后退'));
  expect(html.indexOf('历史前进')).toBeLessThan(html.indexOf('全局资源与命令检索'));
  for(const label of ['左侧导航','右侧属性','底部面板'])expect(html.indexOf(`收起或展开${label}`)).toBeGreaterThan(html.indexOf('titlebar-right'));
});
it('标题栏空白拖动阻止文本选择，交互控件不触发窗口操作',()=>{
  const operations:string[]=[];const noop=()=>{};
  const header=WorkbenchTitlebar({platform:'macos',canBack:false,canForward:false,back:noop,forward:noop,toggle:noop,search:noop,operate:action=>operations.push(action)});
  let prevented=false;
  header.props.onPointerDown({button:0,detail:1,target:{closest:()=>null},preventDefault:()=>{prevented=true;}});
  expect(operations).toEqual(['drag']);expect(prevented).toBe(true);
  header.props.onPointerDown({button:0,detail:1,target:{closest:()=>({})},preventDefault:noop});
  expect(operations).toEqual(['drag']);
  header.props.onPointerDown({button:0,detail:2,target:{closest:()=>null},preventDefault:noop});
  expect(operations).toEqual(['drag','maximize']);
});
