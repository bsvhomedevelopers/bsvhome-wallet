import SwiftUI

private struct BsvMultisigRefreshModifier: ViewModifier {
    @Environment(\.scenePhase) private var scenePhase
    @State private var isVisible = false
    let automatically: Bool
    let action: @MainActor @Sendable () async -> Void

    private var isActive: Bool { automatically && isVisible && scenePhase == .active }

    func body(content: Content) -> some View {
        content
            .task(id: isActive) {
                guard isActive else { return }
                await action()
                while !Task.isCancelled {
                    do { try await Task.sleep(for: .seconds(30)) }
                    catch { return }
                    guard !Task.isCancelled else { return }
                    await action()
                }
            }
            .refreshable { await action() }
            .onAppear { isVisible = true }
            .onDisappear { isVisible = false }
    }
}

extension View {
    func bsvMultisigRefresh(automatically: Bool = true, action: @MainActor @Sendable @escaping () async -> Void) -> some View {
        modifier(BsvMultisigRefreshModifier(automatically: automatically, action: action))
    }
}
