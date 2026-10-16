import type { DraftInput, DraftPreview } from "../../shared/api/contracts";
import { ModelDraft, type InputRow } from "./pointDraft";

/** 追加的确认边界：异步完整校验后再次比较草稿，取消／失败不写历史。 */
export async function appendValidated(
  target: ModelDraft,
  rows: InputRow[],
  validate: (input: DraftInput) => Promise<DraftPreview>,
  active: () => boolean,
) {
  const revision = target.revision;
  const input = target.input();
  input.points.push(...structuredClone(rows));
  const result = await validate(input);
  if (!active()) return false;
  if (target.revision !== revision)
    throw Error("目标草稿已变化，请重新确认并校验导入。");
  if (!result.model) throw result.errors[0] ?? Error("导入校验失败");
  target.change((state) => state.rows.push(...structuredClone(rows)));
  return true;
}
