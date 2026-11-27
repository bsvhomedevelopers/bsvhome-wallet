// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import protocol BsvHomeCore.BsvHomeChainSettingsServiceProtocol
import Localization
import Primitives
import PrimitivesComponents
import Style
import SwiftUI

@MainActor
@Observable
final class AddNodeSceneViewModel: Identifiable {
    let id = UUID()
    private let service: any BsvHomeChainSettingsServiceProtocol

    let chain: Chain

    var urlInputModel = InputValidationViewModel(mode: .onDemand)
    var state: StateViewType<AddNodeResultViewModel> = .noData
    var isPresentingScanner: Bool = false
    var isPresentingAlertMessage: AlertMessage?
    private(set) var loadTrigger: AddNodeLoadTrigger?
    private(set) var isImporting = false
    private var checkRevision = UUID()
    private var checkedInput: String?

    var canImport: Bool {
        !isImporting && state.value != nil && checkedInput == urlInputModel.text
    }

    var nodeCheckDebounce: Duration {
        .milliseconds(service.nodeCheckDebounceMilliseconds())
    }

    init(chain: Chain, service: any BsvHomeChainSettingsServiceProtocol) {
        self.chain = chain
        self.service = service
    }

    var title: String {
        Localized.Nodes.ImportNode.title
    }

    var actionButtonTitle: String {
        Localized.Wallet.Import.action
    }

    var inputFieldTitle: String {
        Localized.Common.url
    }

    var errorTitle: String {
        Localized.Errors.errorOccurred
    }

    var chainModel: ChainViewModel {
        ChainViewModel(chain: chain)
    }

    var warningModel: ListItemModel {
        ListItemModel(
            title: Localized.Asset.Verification.warningTitle,
            titleStyle: .headline,
            titleExtra: Localized.Nodes.ImportNode.warningMessage,
            titleStyleExtra: .bodySecondary,
            imageStyle: ListItemImageStyle(
                assetImage: AssetImage(type: .emoji(Emoji.WalletAvatar.warning.rawValue)),
                imageSize: .image.semiMedium,
                alignment: .top,
                cornerRadiusType: .none,
            ),
        )
    }
}

// MARK: - Business Logic

extension AddNodeSceneViewModel {
    func onChangeInput() {
        guard loadTrigger?.url != urlInputModel.text else { return }
        setLoadTrigger(isImmediate: false)
    }

    func setInput(_ text: String) {
        urlInputModel.text = text
        setLoadTrigger(isImmediate: true)
    }

    private func setLoadTrigger(isImmediate: Bool) {
        let text = urlInputModel.text
        let next = text.isEmpty ? nil : AddNodeLoadTrigger(url: text, isImmediate: isImmediate)
        guard loadTrigger != next else { return }
        checkRevision = UUID()
        checkedInput = nil
        state = .noData
        loadTrigger = next
    }

    func importFoundNode() async throws {
        guard canImport, let input = checkedInput, case let .data(model) = state else { throw CancellationError() }
        let revision = checkRevision
        isImporting = true
        defer { isImporting = false }
        try Task.checkCancellation()
        try await service.addNode(chain: chain.rawValue, url: model.url)
        try Task.checkCancellation()
        guard revision == checkRevision, input == urlInputModel.text else { throw CancellationError() }
    }

    func load() async {
        let input = urlInputModel.text
        guard !isImporting, !input.isEmpty else { return }
        let revision = UUID()
        checkRevision = revision
        checkedInput = nil
        state = .loading
        do {
            try Task.checkCancellation()
            let result = try await service.checkNode(chain: chain.rawValue, url: input)
            try Task.checkCancellation()
            guard revision == checkRevision, input == urlInputModel.text else { return }
            checkedInput = input
            state = .data(AddNodeResultViewModel(result: result))
        } catch {
            guard revision == checkRevision, input == urlInputModel.text else { return }
            if Task.isCancelled || error is CancellationError {
                state = .noData
            } else {
                state.setError(error)
            }
        }
    }
}
