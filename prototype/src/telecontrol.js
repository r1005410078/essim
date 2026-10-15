export const categories = ["遥测", "遥信", "遥控", "遥调"];
export const isTelemetry = (category) =>
  category === "遥测" || category === "遥信";
export function pointCategory(p) {
  if (categories.includes(p.category)) return p.category;
  if (/目标|设定|给定/.test(p.name || "")) return "遥调";
  if (/启动|停止|复位|分合/.test(p.name || "")) return "遥控";
  if (/状态|告警|故障|开关/.test(p.name || "") || p.type === "Bool")
    return "遥信";
  return "遥测";
}
export function normalizeGroup(g) {
  return {
    ...g,
    role: g.role?.includes("主站") ? "主站" : "从站",
    points: (g.points || []).map((p) => ({ ...p, category: pointCategory(p) })),
  };
}
export function validateAction(role, category, action, value) {
  if (!categories.includes(category)) return "未知四遥类型";
  if (!Number.isFinite(value)) return "请输入有限数值";
  if (
    (category === "遥信" || category === "遥控") &&
    value !== 0 &&
    value !== 1
  )
    return `${category}仅允许 0 或 1`;
  const allowed =
    role === "从站"
      ? isTelemetry(category)
        ? "置数"
        : "接收"
      : isTelemetry(category)
        ? "接收"
        : "发送";
  if (action !== allowed) return `${role}的${category}不允许${action}`;
  return "";
}
export function findPeerPoint(point, points) {
  const matches = points.filter(
    (p) =>
      pointCategory(p) === pointCategory(point) &&
      String(p.address) === String(point.address),
  );
  if (!matches.length) throw Error("对端找不到同四遥、同地址的数据点");
  if (matches.length > 1) throw Error("对端地址重复，无法唯一匹配");
  return matches[0];
}
