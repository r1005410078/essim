export interface Command<C> {
  id: string;
  title: string;
  category?: string;
  keywords?: readonly string[];
  enabled?: (context: C) => boolean;
  run: (context: C) => unknown | Promise<unknown>;
}
/** 借鉴VS Code的命令ID/元数据/执行分离；不引入扩展宿主和when表达式解释器。 */
export class CommandRegistry<C> {
  private commands = new Map<string, Command<C>>();
  register(command: Command<C>) {
    if (this.commands.has(command.id)) throw Error(`命令已注册：${command.id}`);
    this.commands.set(command.id, command);
    return () => { if (this.commands.get(command.id) === command) this.commands.delete(command.id); };
  }
  list(context: C, query = "") {
    const tokens = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
    return [...this.commands.values()].filter(command => {
      const text = [command.id, command.title, command.category, ...(command.keywords ?? [])].join(" ").toLocaleLowerCase();
      return tokens.every(token => text.includes(token));
    }).map(command => ({id:command.id,title:command.title,category:command.category,enabled:command.enabled?.(context) ?? true}));
  }
  async execute(id: string, context: C) {
    const command = this.commands.get(id);
    if (!command) throw Error(`命令不存在：${id}`);
    if (command.enabled && !command.enabled(context)) throw Error(`命令当前不可用：${command.title}`);
    return await command.run(context);
  }
}
