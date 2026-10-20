import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return false
  }

  override func applicationShouldHandleReopen(
    _ sender: NSApplication, hasVisibleWindows _: Bool
  ) -> Bool {
    guard let window = mainFlutterWindow else { return true }
    if window.isMiniaturized {
      window.deminiaturize(sender)
    }
    window.makeKeyAndOrderFront(sender)
    return false
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
