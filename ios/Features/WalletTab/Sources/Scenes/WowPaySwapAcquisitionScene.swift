import Store
import SwiftUI

public struct WowPaySwapAcquisitionScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @State private var model: WowPayPortalSwapViewModel
    private let walletModel: WalletSceneViewModel

    public init(model: WowPayPortalSwapViewModel, walletModel: WalletSceneViewModel) {
        _model = State(initialValue: model)
        self.walletModel = walletModel
    }

    public var body: some View {
        WowPaySwapContentView(model: model, walletModel: walletModel, presentation: .acquisition)
            .bindQuery(walletModel.fixedAssetsQuery)
            .task(id: model.requestVersion) {
                await model.load()
            }
            .onChange(of: scenePhase) { _, phase in
                if phase == .background {
                    model.onDisappear()
                } else if phase == .active {
                    Task {
                        await model.refresh()
                    }
                }
            }
    }
}
