# 资源树状态图标

节点名称靠左，状态图标放在节点行最右侧，以12／13px彩色线性图标显示，多个标记间距4px。名称可截断，图标不收缩；不增加行内状态文字，不改变28px行高、原有业务图标、展开、选择及菜单。hover使用公共Tooltip，行的aria-description同时提供说明。

| 范围 | 状态 | 图标及颜色 |
| --- | --- | --- |
| TCP从站 | 监听中 | 绿色Radio，提示当前连接数 |
| RTU从站 | 串口应答中 | 绿色Plug |
| 主站 | 已连接／周期采集 | 绿色Plug／RefreshCw |
| 端点 | 启动中、停止中、连接中、重连等待 | 蓝色LoaderCircle／RefreshCw |
| 端点 | 未启动、已停止、未连接 | 灰色Square／Unplug |
| 端点 | 运行失败／连接失败 | 红色CircleAlert，显示真实原因 |
| 端点 | 查询失败或尚未确认 | 灰色CircleHelp，不延续绿色 |
| Unit | 已禁用／已暂停采集 | 灰色Ban／PauseCircle |
| 主站Unit | 已观察点过期或失败 | 琥珀色TriangleAlert，注明仅为最近查询点 |
| 配置 | 待应用／未保存 | 琥珀色Clock／蓝色Dot，与运行状态并列 |

从站启用Unit不复制端点运行图标，不能从“监听中”推导其在线。主站Unit只有存在已应用且启用的单元、主站已连接并开启周期采集时才显示采集图标。暂停、禁用和已观察异常优先展示。未保存Unit点表修改目前汇总到所属端点，模型未保存标记显示在资源树模型节点。

`TreeStatusLayer`通过树的公开getFileTreeContainer取得Shadow DOM，按锁定版本的data-item-path追加自有容器；React portal渲染状态，MutationObserver跟随虚拟行复用、折叠、删除，清理自有容器及说明。升级@pierre/trees时需复核该边界，不修改第三方包，也不重建树模型。

只对当前实际渲染的树行所对应端点，每秒读取一次现有主从站元数据，最多4并发，unit:null、points:[]。隐藏导航／页面时不发送查询，失效代次、配置序号变化和删除后的响应被忽略。质量图标复用运行表已有真实查询，不新增点值读取，不宣称全Unit健康或全点异常汇总。

提案见[show-resource-tree-status](../../openspec/changes/show-resource-tree-status/proposal.md)，验证见[原生及自动化记录](../test-records/2026-10-24-tree-status.md)。

节点菜单仅通过右键或键盘上下文菜单操作打开，使用树组件的`triggerMode: "right-click"`，不再显示行尾三点按钮或占用其动作槽；导航顶部“更多”按钮保留。
