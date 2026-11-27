import Localization
import Style
import SwiftUI

struct ImportWalletProtection: ViewModifier {
    @Environment(\.scenePhase) private var scenePhase
    @State private var isCaptured = false
    let model: ImportWalletSceneViewModel?

    func body(content: Content) -> some View {
        content
            .opacity(scenePhase == .active && !isCaptured ? 1 : 0)
            .overlay {
                if scenePhase != .active || isCaptured {
                    Colors.WowPay.background
                        .ignoresSafeArea()
                        .overlay { Text(Localized.SecretPhrase.ContentHidden.description) }
                }
            }
            .disabled(isCaptured)
            .onChange(of: scenePhase) { _, phase in
                model?.setScreenAvailability(phase == .active && !isCaptured)
                if phase == .background {
                    model?.clearSensitiveState()
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIScreen.capturedDidChangeNotification)) { _ in updateCapture() }
            .onAppear(perform: updateCapture)
    }

    private func updateCapture() {
        let captured = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .contains { $0.traitCollection.sceneCaptureState == .active }
        isCaptured = captured
        model?.setScreenAvailability(scenePhase == .active && !captured)
        if captured {
            model?.clearSensitiveState()
        }
    }
}
