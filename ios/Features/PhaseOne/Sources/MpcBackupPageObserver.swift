import SwiftUI

struct MpcBackupPageObserver: UIViewRepresentable {
    let leave: @MainActor () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(leave: leave)
    }

    func makeUIView(context _: Context) -> UIView {
        UIView(frame: .zero)
    }

    func updateUIView(_: UIView, context _: Context) {}

    static func dismantleUIView(_: UIView, coordinator: Coordinator) {
        coordinator.leave()
    }

    @MainActor
    final class Coordinator {
        let leave: @MainActor () -> Void

        init(leave: @escaping @MainActor () -> Void) {
            self.leave = leave
        }
    }
}
