/** 手动数据适配层测量，不代表WebView渲染或Modbus吞吐。使用pnpm exec jiti scripts/measure-m02.ts。 */
import { ModelDraft, defaultRow } from "../src/features/workspace/pointDraft";
import { DraftSource } from "../src/features/workspace/DraftSource";
import { pastePointText } from "../src/features/workspace/pointClipboard";
import { fields } from "../src/features/workspace/pointDraft";
async function main() {
  for (const count of [1000, 10000]) {
    const samples = [];
    for (let round = 0; round < 4; round++) {
      const start = performance.now();
      const draft = new ModelDraft(
        "measure",
        "测量",
        Array.from({ length: count }, (_, i) => ({
          ...defaultRow(i % 2 ? "measurement" : "indication", `p${i}`),
          address: String(i),
        })),
      );
      const source = new DraftSource(draft, "measurement");
      const constructed = performance.now();
      const col = fields.indexOf("note");
      await pastePointText(
        source,
        () => source,
        {
          activeCell: { rowIndex: 0, colIndex: col },
          anchorCell: null,
          extentCell: null,
          selectedRange: { startRow: 0, endRow: 0, startCol: col, endCol: col },
        },
        Array.from({ length: 500 }, () => "批量粘贴测量").join("\n"),
      );
      const pasted = performance.now();
      const other = new DraftSource(draft, "indication");
      const filtered = performance.now();
      draft.undo();
      const end = performance.now();
      if (draft.dirty || other.getRowCount() !== count / 2 + 30)
        throw Error("测量后数据不一致");
      if (round)
        samples.push({
          constructMs: constructed - start,
          paste500Ms: pasted - constructed,
          filterMs: filtered - pasted,
          undoMs: end - filtered,
          rssMiB: process.memoryUsage().rss / 1048576,
        });
    }
    console.log(JSON.stringify({ count, samples }));
  }
}
void main();
