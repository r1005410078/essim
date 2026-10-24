import AppKit
import CoreGraphics
import ApplicationServices

// 独立测试壳专用；没有其他应用回退路径，坐标按截图像素 / 2 转为窗口逻辑坐标。
let args = CommandLine.arguments
let apps = NSRunningApplication.runningApplications(withBundleIdentifier: "dev.essim.dockingnative")
guard apps.count == 1, let app = apps.first else { fatalError("独立测试壳进程不唯一") }
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
let candidates = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == app.processIdentifier && ($0[kCGWindowLayer as String] as? Int) == 0 && ($0[kCGWindowName as String] as? String) == "ESSim · 储能协议仿真工作台" }
guard candidates.count == 1, let entry = candidates.first, let dict = entry[kCGWindowBounds as String] as? [String:Any], let bounds = CGRect(dictionaryRepresentation: dict as CFDictionary) else { fatalError("测试壳可见主窗口不唯一") }
print("pid=\(app.processIdentifier) window=\(bounds)")
if args.count == 1 { exit(0) }
guard args.count == 5, AXIsProcessTrusted() else { fatalError("需要四个截图坐标参数及现有辅助功能权限") }
let values = args.dropFirst().map { Double($0)! / 2 }
let from = CGPoint(x: bounds.minX + values[0], y: bounds.minY + values[1])
let to = CGPoint(x: bounds.minX + values[2], y: bounds.minY + values[3])
guard bounds.contains(from), bounds.contains(to) else { fatalError("拒绝测试窗口外的鼠标操作") }
app.activate(options: [])
Thread.sleep(forTimeInterval: 0.2)
let source = CGEventSource(stateID: .hidSystemState)!
func send(_ type: CGEventType, _ point: CGPoint) {
 let event = CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: point, mouseButton: .left)!
 event.post(tap: .cghidEventTap)
}
send(.mouseMoved, from); send(.leftMouseDown, from)
for i in 1...20 {
 Thread.sleep(forTimeInterval: 0.016)
 let t = Double(i) / 20
 send(.leftMouseDragged, CGPoint(x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t))
}
send(.leftMouseUp, to)
print("drag \(from) -> \(to)")
