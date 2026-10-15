export const defaultScript = `// 离线策略实验 · 正功率放电，负功率充电
function on_start(ctx) {
  ctx.log('策略初始化');
}

function on_tick(ctx) {
  const soc = ctx.read('BMS_01.SOC');
  const hour = ctx.time.hour;
  if (hour < 7 && soc < ctx.params.upper) {
    ctx.command('PCS_01.目标功率', -ctx.params.maxPower);
  } else if (hour >= 17 && hour < 21 && soc > ctx.params.lower) {
    ctx.command('PCS_01.目标功率', ctx.params.maxPower);
  } else {
    ctx.command('PCS_01.目标功率', 0);
  }
}

function on_stop(ctx) {
  ctx.log('策略运行完成');
}
`;
export function simulate(source, parameters = {}) {
  const params = {
    capacity: 1000,
    soc: 50,
    lower: 20,
    upper: 90,
    maxPower: 250,
    ...parameters,
  };
  if (!Number.isFinite(params.capacity) || params.capacity <= 0)
    throw Error("容量必须大于 0");
  if (
    ![params.soc, params.lower, params.upper, params.maxPower].every(
      Number.isFinite,
    ) ||
    params.lower < 0 ||
    params.upper > 100 ||
    params.lower >= params.upper ||
    params.soc < params.lower ||
    params.soc > params.upper ||
    params.maxPower <= 0
  )
    throw Error("检查 SOC 范围、初始 SOC 和功率参数");
  const hooks = new Function(
    `${source}\nreturn {start:typeof on_start==='function'?on_start:null,tick:typeof on_tick==='function'?on_tick:null,stop:typeof on_stop==='function'?on_stop:null};`,
  )();
  if (!hooks.tick) throw Error("脚本必须定义 on_tick(ctx)");
  let soc = params.soc,
    power = 0;
  const logs = [],
    samples = [];
  const step = 5 / 60;
  const ctx = {
    params,
    time: { hour: 0, minutes: 0 },
    read(point) {
      if (point === "BMS_01.SOC") return soc;
      if (point === "PCS_01.实际功率") return power;
      throw Error(`未绑定的数据点：${point}`);
    },
    command(point, value) {
      if (point !== "PCS_01.目标功率") throw Error(`未绑定的控制点：${point}`);
      if (!Number.isFinite(value)) throw Error("目标功率必须是有限数值");
      power = Math.max(-params.maxPower, Math.min(params.maxPower, value));
    },
    log(message) {
      if (logs.length < 500)
        logs.push({ time: ctx.time.minutes, message: String(message) });
    },
  };
  hooks.start?.(ctx);
  for (let i = 0; i < 288; i++) {
    ctx.time = { hour: Math.floor(i / 12), minutes: i * 5 };
    hooks.tick(ctx);
    if (power > 0)
      power = Math.min(
        power,
        Math.max(0, ((soc - params.lower) * params.capacity) / 100 / step),
      );
    else
      power = Math.max(
        power,
        -Math.max(0, ((params.upper - soc) * params.capacity) / 100 / step),
      );
    soc = Math.max(
      params.lower,
      Math.min(params.upper, soc - ((power * step) / params.capacity) * 100),
    );
    samples.push({ hour: (i + 1) / 12, soc, power });
  }
  hooks.stop?.(ctx);
  return { samples, logs, params };
}
