# 验收矩阵与审查闭环

状态：G0已取得受限验证证据（2026-10-25），见[G0记录](../../../docs/test-records/2026-10-25-protocol-g0.md)；仅1.x／2.x完成，G_A／G_B／G_C尚未执行。提案完整性校验不等于功能／协议验收。任务入口见[tasks](tasks.md)，设计见[design](design.md)。

## 阶段门槛与失败条件

| 门槛 | 必须具备的通过证据 | 失败时边界 | 任务 |
| --- | --- | --- | --- |
| G0：冻结接口前 | 六项反例通过；104实际TCP原生队列／窄监控接口可行；61850模型／服务反例保留身份和依赖；固定候选契约及保证矩阵 | 不冻结接口、不进入生产迁移；补证据或修订方案，不能降为fixture代替真实队列 | 1.1–2.8 |
| G_A：通信管理兼容重构 | 四种Modbus profile、旧IPC／UI／headless、全部生命周期、可靠Journal及格式1回归；实际macOS工作台证据 | 停留在上一兼容边界，不进入共享状态；不将格式升级和新写路径夹带进首次迁移 | 3.1–4.7 |
| G_B：共享状态与原子应用 | 唯一提交决策、投影回执、ScopeFence全写入口排空、最终快照迁移、动作仲裁；真实Modbus＋104共享Example及故障隔离；G_A持续通过 | 不暴露共享能力为正式可用，不进入格式2；未知决策不得强行解除隔离 | 5.1–7.6 |
| G_C：持久化与展示 | 显式格式2升级／恢复／未知内容保护、只读元数据和通用观测UI；G_A／G_B全回归 | 不发布新格式／相关UI能力，不用前端成功抵消持久故障 | 8.1–8.6 |
| 最终核对 | 六项及原13项、生命周期和证据范围完整，旁路已移除，未验门槛明确 | 不标本提案整体完成，也不宣称新协议完整产品已交付 | 9.1–9.4 |

G0只搭建最小验证设施，不要求先实现全部管理器／状态／工程系统。104窄监控接口由默认关闭feature隔离并已按G0复核验证，vendor及生产适配器未修改。G_A可独立验收，G_B可独立验收；G_C单独处理持久化及展示。

## 四项G0复核修复与重新冻结条件

| 遗漏 | 必须通过的用例／接口约定 | 对应任务 |
| --- | --- | --- |
| STARTDT识别窄于上游 | all_upstream_startdt_reset_forms_invalidate_the_old_session覆盖保留字节、组合位及非标准长度；testfr_branch_does_not_invalidate_a_live_session证明分支优先级；不得认可非法帧为标准报文 | 2.5–2.6 |
| 只保证入队后的FIFO | changes_batches_are_contiguous_atomic_and_keep_multiple_frames_per_commit验证after／through接续和多帧整批预算；fifo_rejects_postcut_regression_and_stale_snapshot及late_postcut_changes_and_old_snapshot_cannot_roll_back_real_tcp_values覆盖12→11、旧快照与真实线上无回退 | 2.6 |
| Fence跨轮复用 | old_fence_cannot_abort_inspect_or_publish_a_new_quiesce验证f1结束后不能读取、取消或发布f2，结束后的重复调用失效；独立quiesce身份与schema epoch分开 | 2.3 |
| 会话变化不可主动观察／恢复证据不足 | session_is_current不发ASDU；session_query_detects_idle_reset_without_sending_an_asdu及idle_empty_fifo_reset_still_captures_and_sends_a_new_snapshot验证空FIFO；pending_projection_restarts_from_fresh_snapshot_on_all_session_transitions对断线、STOPDT、重复／非标准STARTDT和接管验证双层积压、旧结果拒绝、新快照7先于变化8 | 2.5–2.6 |

四项全部通过后才可重新确认2.8。生产管理器、真正共享状态Example、G_A／G_B／G_C仍未实现。当前复核结果与原始输出见[G0记录](../../../docs/test-records/2026-10-25-protocol-g0.md)，不能只以原19项或OpenSpec语法校验恢复完成标记。

### 第二轮复核补充门槛（本次两项修复）

| 遗漏 | 必须通过的用例／接口约定 | 对应任务 |
| --- | --- | --- |
| Gap重建FIFO清掉高水位 | gap_resync_rejects_late_snapshots_and_preserves_progress_across_session_reset：包含未发cursor13；Gap后连续拒绝cut10、cut12及错误epoch；同会话和STARTDT重置均保留(1,13)，失败期间不发送／不接纳变化；快照13后变化14的实际线上为1→3→4→5 | 2.6、2.8 |
| 记录受理前消耗序号 | record_queue_rejection_preserves_sequence_and_recovers_after_budget_release：真实RecordingPool额度连续拒绝两次，无Pending／值／游标／投影变化，序号和可靠前缀仍为1；释放额度后显式新提交完成，连续两次提交可靠前缀到5、状态游标到2 | 2.2、2.8 |

原28项通过不足以覆盖这两项；最新30项及两项修复前失败输出均须保留，全部通过再确认G0。记录恢复只涵盖受理前临时额度拒绝，不解除已受理后故障／Unknown隔离；状态高水位按所有者维护，不因传输会话更新清零。G_A仍未授权。

## 六项二次审查对应

| 问题 | 边界修订 | 规范 | 任务／关键验收 |
| --- | --- | --- | --- |
| P1 停止被业务门阻断 | D3、D5：DataPermit／LifecycleLease分离，先锁存，内部复用上下文，不夺取原许可 | protocol-communication-management | 2.1、3.2–3.4、4.4：在途超时占门且队列满仍能请求停止；错误凭证拒绝；真实释放／Finalized分开 |
| P1 提交／审计／投影次序模糊 | D7、D10：异步受理；ScopeJournal唯一决策后原子发布；每Binding回执 | protocol-state-and-controls、protocol-observation-and-recording | 2.2、5.3–5.6、7.3–7.4：确认前无可见新值／投影，未知不重发，StateCommitted不等于协议成功，LegacyRaw无双重Journal |
| P1 ApplyImpact缺少状态写屏障 | D7、D13：StateRegistry独立，ScopeFence封闭／排空／最终快照／新epoch | protocol-state-and-controls | 2.3、5.1、6.2–6.5：四类写入口全部拦截，已受理结果不丢失，未知阻塞迁移，迟到旧句柄拒绝 |
| P1 104原生队列破坏总召排序 | D8、D11：每会话单应用FIFO／同原生路径；窄监控接口先验证 | protocol-state-and-controls、protocol-adapter-conformance | 2.5–2.6、7.1–7.2：窗口堵塞＋旧自发积压＋总召分批＋新变化，无跨批次回退；队列满重试不重复；重连失效旧会话，无SOE承诺 |
| P2 保存与应用校验依据混用 | D3、D12–D13：SavedDefinition／AppliedSnapshot／RuntimeSnapshot分开 | protocol-model-boundaries | 1.2、2.4、6.4–6.5、8.1：保存B未应用仍能启动自包含A，显式应用B后旧token拒绝，候选应用变化重新预览 |
| P2 第二协议验证太晚 | D14、Migration Plan：G0→G_A→G_B→G_C | protocol-adapter-conformance | 2.8、4.7、7.6、8.6、9.3：前一门失败不得推进，G_A不依赖格式2或共享状态交付，反例不留到全部迁移后 |

## 提交与迁移故障矩阵

以下共享提交用例在G0验证候选契约，在G_B对真实实现复测；不能把模拟设施结果计作实现完成。

| 注入位置／时序 | 必须观察到 | 禁止结果 |
| --- | --- | --- |
| 类型／权限／转换整批校验失败 | 整批拒绝，旧值／游标不变，无投影 | 部分值提交或数值默默舍入 |
| 意图在受理前因临时记录队列额度不足拒绝 | 无Pending／值／游标／投影变化，不消耗连续记录序号；额度释放后显式新提交可正常接续 | 序号断档后永久拒绝，或自动重发被拒请求／解除Unknown |
| CommitIntent或CommitDecided确定未可靠写入 | 旧可见值，失败定位，scope故障门禁，无新投影 | 返回StateCommit后再补业务决策 |
| CommitDecided可能落盘但确认丢失 | Unknown、最后确认视图、保留操作资源、阻止冲突写／迁移／新投影 | 猜测成功／失败、重新提交同一业务动作 |
| 决策可靠后、内存发布前中断 | 可靠决策及未完成阶段可识别；受控恢复查询 | 断言从未提交，或自动恢复网络／重放控制 |
| 原子发布后某Binding投影拒绝 | 原业务提交保留，各投影回执分别标明故障／滞后 | 回滚其他协议已有效果，伪造源协议成功 |
| 所有协议停止后提交 | ScopeJournal由StateRegistry继续拥有，可正常提交 | 依赖旧RunId、复位值或丢失审计 |
| 预览后、Fence前已有写入／动作 | 排空后最终快照含其确定结果 | 用预览旧值迁移覆盖新值 |
| Fence后四类新写入 | 本地补丁、远端写、绑定采样、DeviceCommand均ScopeQuiescing | 只拦网络却允许UI或动作继续改值 |
| 已受理提交／动作／投影未完成 | 安全取消余项或继续排空，未知不能视为Quiescent | 仅凭通信ResourcesReleased就迁移 |
| 迁移配置确定失败 | 旧schema／应用保留，可安全abort准入，协议保持真实停止状态 | 半发布或自动重启掩盖停机 |
| 配置持久决策未知 | RecoveryRequired，查询／停止／恢复可用，写入与迁移阻塞 | 强制unfence、从未确认配置继续运行 |
| 新epoch后旧回调到达 | 旧句柄拒绝；重新获取绑定后才可提交 | 旧结构写入新schema |

LegacyRawScope另跑原Modbus可靠前缀／结果未知矩阵，不叠加ScopeJournal，不提前发布C Mapping已变但记录未确认的值；共享路径复测局部寄存器合成、非法浮点等原校验。

## 104原生排序门槛

G0使用固定上游版本、实际安全封装、真实TCP和独立字节驱动，留存ASDU接纳顺序与线缆接收顺序：

- 先复现回调send／confirm／finish高优先级与Server.enqueue低优先级混排反例，确认游标不能修正已入C队列的旧帧。
- 候选方案所有总召／自发数据走同一会话FIFO与同一高优先级发送路径；在库队列非空、发送窗口饱和和大量总召跨tick时，旧已接纳帧→完整总召批次→切点后变化的顺序成立。快照／订阅切点及批次位置原子保留；快照cut不小于接纳高水位，ChangeBatch的after精确接续、有进展时through前进，多帧整批准入；当前epoch／位置空轮询不触发Gap；乱序、重叠、断档和旧快照均进入Gap，不能后插。
- 队列WouldBlock只保留队首并推进事件循环；已接纳帧不能重发入队，失败不能偷偷绕到低优先级路径；应用FIFO达到限额须拒绝／标失步。
- 回调复制受限自有请求数据后返回，借用Request／C连接不逃逸；SessionToken绑定所有者及代际，断线／停止／旧token调用被安全拒绝；每tick后可用session_is_current在不发送ASDU时检查重置，检测条件覆盖上游实际重置分支。
- 断连释放真实C会话，新会话以快照重同步，旧未发应用投影标为被替代／缺口，C内缓冲数量不被推测；带积压／空FIFO时都先捕获当前快照再接续变化，不伪造清队列、Sent／Acked、历史SOE或离线无损。

若上述策略在已验证上游API下不可实现，G0失败，必须修订策略／能力契约再冻结接口；不能将该问题推给G_B结束时处理。G_B再次用同样的原生反例验证管理器与真实共享状态集成。

## 保存、应用、启动的权威依据

| 操作／竞态 | 校验依据与期望 |
| --- | --- |
| 保存模型B但应用仍为A，再启动 | 管理器解析A的AppliedPlanToken及自包含资产；A仍能启动，B不混入运行 |
| A源文件被修改或替换 | 使用已应用A的资产副本／摘要；不可重读最新源文件替换A，也不因普通保存错误废止A |
| 应用预览B后SavedDefinition／依赖变化 | 候选应用提交拒绝并重新预览，原应用A不变 |
| B成功显式应用后旧A凭证启动 | 拒绝旧token并获取当前应用；不接受调用方自造PreparedPlan |
| 运行中保存C | RuntimeSnapshot保持本次运行内容；保存不修改在途运行或自动应用 |

内部token／epoch／摘要用于当前身份和并发隔离，不提供历史选版、升级或expectedRevision。

## 原13项深度审查对应

| 审查项 | 设计 | 规范文件 | 实施任务／关键验收 |
| --- | --- | --- | --- |
| P1-01 权威设备值 | D7 | protocol-state-and-controls | 2.2–2.3、5.1–5.6、7.3；旧Unit不合并、项目拥有共享状态、停止协议不复位 |
| P1-02 实例／资源／对象身份 | D2、D4、D6 | protocol-model-boundaries、protocol-communication-management | 2.7、3.4–3.5；无连接服务、共享使用者和非独占拨号 |
| P1-03 原生模型 | D2、D8、D12 | protocol-model-boundaries | 2.7、7.5、8.1、8.4；结构／数组／FC／DataSet顺序和未绑定节点不丢失 |
| P1-04 控制独立 | D9 | protocol-state-and-controls | 6.1；选择不改值、测试无动作、同目标仲裁及未知不重发 |
| P1-05 原生上送机制 | D7–D8 | protocol-state-and-controls | 2.5–2.7、5.6、7.1–7.5；质量／同值原因、不复制RCB引擎、接收范围明确 |
| P1-06 快照／流切点 | D8 | protocol-state-and-controls | 2.6、5.6、7.1–7.2；真实积压排序、Gap及部分报告不清零 |
| P1-07 记录保证 | D7、D10 | protocol-observation-and-recording | 2.2、4.6、5.3–5.5、7.4–7.5；唯一决策、原保证、未知不重发、捕获缺口 |
| P1-08 应用影响 | D13 | protocol-state-and-controls | 2.3、6.2–6.5、8.2；完整依赖、ScopeFence、最终快照、未知门禁 |
| P1-09 原生公共符号 | D11 | protocol-adapter-conformance | 7.5、9.2；缺失证据拒绝，双原生共存由后续启用门槛检查 |
| P2-10 质量／时间 | D7、D10 | protocol-observation-and-recording | 5.2、5.6、7.4；精度、源时标／观测时间分开、对时不改宿主 |
| P2-11 回调线程／延迟 | D3、D9、D11 | protocol-adapter-conformance | 2.5–2.6、3.4、7.1–7.2、7.5；借用不逃逸、停止不死锁、无安全控制回复则拒绝 |
| P2-12 两个配置平面 | D8、D12 | protocol-model-boundaries、protocol-state-and-controls | 2.4、7.5、8.1–8.2；会话不自动保存，协议计数不当用户配置历史 |
| P2-13 UI／资源／服务范围 | D4、D6、D12、D14 | protocol-communication-management、protocol-observation-and-recording | 3.5、4.5–4.7、7.6、8.3–8.6；能力门禁、分页、计费与证据分层 |

## 生命周期入口矩阵

| 入口 | 正常路径 | 必须覆盖的反例 |
| --- | --- | --- |
| 手动启停 | 已应用凭证→管理器→适配器→真实释放 | 保存未应用、能力不可用、占门＋满队列、记录失败、错误生命周期上下文 |
| 批量 | 固定范围／公共凭证逐项调度 | 取消余项、未知、手动冲突、内部stop不重入自身门、单项不误停其他项 |
| 应用／删除 | ApplyImpact→ScopeFence→必要释放／排空→最终快照→复核／可靠发布 | 预检失败不先停、保存竞态、写入竞态、部分释放失败、决策未知 |
| 复位／置数 | LegacyRaw原路径或显式共享异步提交→独立投影 | 批量非法、原Unit独立、Fence拒绝、权威已提交而投影失败 |
| 关工程 | 关闭scope准入→管理器停止→操作／记录收尾→状态所有者释放 | 慢回调、待确认操作、SQLite降级、无协议仍有scope记录、其他窗口运行 |
| 全应用退出 | 汇总各工程资源与完成／保存状态 | 任一未确认不提前销毁C模型／回调，不伪造全部已结束 |

## 证据层次及检查入口

| 对象 | 本重构需取得的证据 | 不包含 |
| --- | --- | --- |
| Modbus生产 | 既有四种profile经管理器的独立TCP／PTY、记录故障、UI和工程回归 | 新平台、RS-485硬件或扩大规模承诺 |
| 104监控验证 | 实际lib60870安全层＋真实TCP字节驱动、统一管理器／共享Example | 正式104工作台、控制／时标／TLS、SOE无损或成熟栈／EMS完整互操作 |
| 61850模型／服务 | 非生产fixture的模型、FC、集合、控制及依赖契约 | 真实MMS、GOOSE、SV、C回调时序及双库ABI |
| 后续profile启用 | 原生封装、静态／动态共存、目标平台、独立互操作和业务证据 | 用合成契约替代原生验收 |

实施时先核对命令、feature及依赖仍真实存在，再执行适当检查。G0实际执行命令、结果及未验限制见上述记录；后续阶段须重新按实际范围验收。

```sh
cargo test -p essim-core --offline
cargo test -p essim-infrastructure --features tcp-runtime,vendored --offline
cargo test -p lib60870-rs --offline
pnpm --filter @essim/workbench test
pnpm --filter @essim/workbench build
```

既有独立脚本入口：`scripts/verify_tcp_slave_runtime.py`、`scripts/verify_tcp_master_runtime.py`、`scripts/verify_rtu_slave_runtime.py`、`scripts/verify_rtu_master_runtime.py`。先按脚本真实帮助／现有说明构建对应Example，不臆造统一CLI。G0原生排序探针及61850受限反例已创建并通过；管理器共享状态Example仍未实现，不能报告已通过。

每条证据记录日期、提交／工作区状态、系统架构、链接模式、实例／连接／点数量、频率、持续时间、原始输出及失败／缺口。慢盘／队列故障用确定性注入，真实通信逐步增加独立协议栈／EMS证据；字节驱动和自有两端互测保留覆盖边界。阶段门槛和每条任务只有实际验收后才能打勾。
