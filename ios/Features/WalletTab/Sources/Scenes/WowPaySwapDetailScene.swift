import Localization
import Primitives
import SwiftUI

public struct WowPaySwapDetailScene: View {
    @Environment(\.scenePhase) private var scenePhase
    @State private var model: WowPayPortalSwapViewModel
    @State private var didSelectEntryAsset = false
    private let walletModel: WalletSceneViewModel
    private let entryAsset: AssetId
    private let isActive: Bool

    public init(model: WowPayPortalSwapViewModel, walletModel: WalletSceneViewModel, entryAsset: AssetId, isActive: Bool) {
        _model = State(initialValue: model)
        self.walletModel = walletModel
        self.entryAsset = entryAsset
        self.isActive = isActive
    }

    public var body: some View {
        WowPaySwapContentView(
            model: model,
            walletModel: walletModel,
            presentation: .detail,
            entryAssetNotice: model.isEntryAssetUnsupported ? Localized.wowpaySwapEntryAssetUnsupported : nil,
        )
        .navigationTitle(Localized.Wallet.swap)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.visible, for: .navigationBar)
        .toolbar(.hidden, for: .tabBar)
        .task(id: isActive ? model.requestVersion : nil) {
            guard isActive else { return }
            if !didSelectEntryAsset {
                model.selectEntryAsset(entryAsset)
                didSelectEntryAsset = true
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
