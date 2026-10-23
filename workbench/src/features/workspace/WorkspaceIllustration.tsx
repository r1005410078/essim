import testbench from "./illustrations/energy-testbench-light.png";
import cabinet from "./illustrations/battery-cabinet-light.png";
import inspector from "./illustrations/point-inspector-light.png";
import observations from "./illustrations/communication-log-light.png";

const illustrations = { testbench, cabinet, inspector, observations };

/** 纯装饰的业务草图；空态说明和可用操作由宿主按真实状态提供。 */
export function WorkspaceIllustration({ kind }: { kind: keyof typeof illustrations }) {
  return <img className={`workspace-illustration workspace-illustration--${kind}`} src={illustrations[kind]} alt="" aria-hidden="true" draggable={false}/>;
}
