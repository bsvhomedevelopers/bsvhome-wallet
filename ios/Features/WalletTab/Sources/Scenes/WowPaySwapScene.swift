import SwiftUI

public struct WowPaySwapScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @State private var model: WowPayPortalSwapViewModel
    @Binding private var homeEntryAsset: WowPayAsset?
    private let isActive: Bool
    private let walletModel: WalletSceneViewModel

    public init(model: WowPayPortalSwapViewModel, walletModel: WalletSceneViewModel, isActive: Bool, homeEntryAsset: Binding<WowPayAsset?> = .constant(nil)) {
        _model = State(initialValue: model)
        _homeEntryAsset = homeEntryAsset
        self.walletModel = walletModel
        self.isActive = isActive
    }

    public var body: some View {
        WowPaySwapContentView(model: model, walletModel: walletModel)
            .task(id: isActive ? model.requestVersion : nil) {
                guard isActive else { return }
                if let homeEntryAsset {
                    model.selectHomeSourceAsset(homeEntryAsset)
                    self.homeEntryAsset = nil
                }
                await model.load()
            }
            .onChange(of: isActive) { _, isActive in
                if !isActive {
                    model.onDisappear()
                }
            }
            .onChange(of: scenePhase) { _, scenePhase in
                if scenePhase == .background {
                    model.onDisappear()
                } else if scenePhase == .active, isActive {
                    Task {
                        await model.refresh()
                    }
                }
            }
    }
}
