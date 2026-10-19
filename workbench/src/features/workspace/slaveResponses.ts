import type { SlaveView } from "../../shared/api/contracts";
/** 操作或查询上下文改变后丢弃旧应答；同一运行内拒绝较旧修订。 */
export class SlaveResponseGate {
  private epoch = 0;
  private view: SlaveView | null = null;
  invalidate() { this.epoch++; this.view = null; }
  ticket() { return this.epoch; }
  accept(ticket: number, next: SlaveView): boolean {
    if (ticket !== this.epoch) return false;
    if (this.view?.recordPath === next.recordPath && BigInt(next.revision) < BigInt(this.view.revision)) return false;
    this.view = next;
    return true;
  }
}
