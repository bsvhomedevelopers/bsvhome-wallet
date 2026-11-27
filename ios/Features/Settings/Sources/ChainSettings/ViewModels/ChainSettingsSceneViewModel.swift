// Copyright (c). Gem Wallet. All rights reserved.

import Formatters
import Foundation
import protocol BsvHomeCore.BsvHomeChainSettingsServiceProtocol
import struct BsvHomeCore.BsvHomeNodeSelection
import enum BsvHomeCore.BsvHomeNodeStatusState
import enum BsvHomeCore.BsvHomeServiceError
import BsvHomeCorePrimitives
import Localization
import Primitives

@Observable
@MainActor
public final class ChainSettingsSceneViewModel {
    private let service: any BsvHomeChainSettingsServiceProtocol
    let chain: Chain

    var selectedExplorer: String?
    var nodeDelete: BsvHomeNodeSelection?
    var explorers: [String]
    private(set) var presentedImportNode: AddNodeSceneViewModel?
    private(set) var isLoading = false
    private(set) var errorMessage: String?
    @ObservationIgnored private(set) var operationTask: Task<Void, Never>?
    private var retryAction: RetryAction?

    private enum NodeAction {
        case reload
        case select(String)
        case delete(BsvHomeNodeSelection)
    }

    private enum RetryAction {
        case explorer(String)
        case nodes(NodeAction)
    }

    var canRetry: Bool { retryAction != nil && !isLoading }

    private let formatter = ValueFormatter.full_US

    private var nodes: [BsvHomeNodeSelection] = []
    private var statusStateByNodeUrl: [String: BsvHomeNodeStatusState] = [:]

    public init(chain: Chain, service: any BsvHomeChainSettingsServiceProtocol) {
        self.chain = chain
        self.service = service
        explorers = service.explorers(chain: chain.rawValue)
        selectedExplorer = service.explorerName(chain: chain.rawValue)
    }

    var title: String {
        chain.networkName
    }

    var nodesTitle: String {
        Localized.Settings.Networks.source
    }

    var nodesModels: [ChainNodeViewModel] {
        nodes.map { node in
            ChainNodeViewModel(
                node: node,
                bsvHomeNodeFlag: service.nodeFlag(url: node.url),
                statusState: statusStateByNodeUrl[node.url] ?? .loading,
                formatter: formatter,
            )
        }
    }

    var explorerTitle: String {
        Localized.Settings.Networks.explorer
    }

    var deleteButtonTitle: String {
        Localized.Common.delete
    }

    func deleteConfirmationTitle(for nodeName: String) -> String {
        Localized.Common.deleteConfirmation(nodeName)
    }

    func canDelete(url: String) -> Bool {
        service.canDeleteNode(chain: chain.rawValue, url: url)
    }
}

// MARK: - Actions

extension ChainSettingsSceneViewModel {
    func load() async {
        if let operationTask {
            await operationTask.value
        } else {
            await start(.reload)?.value
        }
    }

    func onSelectExplorer(name: String) {
        guard !isLoading else { return }
        errorMessage = nil
        retryAction = nil
        do {
            try service.setExplorerName(chain: chain.rawValue, name: name)
        } catch {
            errorMessage = Localized.Errors.errorOccurred
            retryAction = .explorer(name)
        }
        selectedExplorer = service.explorerName(chain: chain.rawValue)
    }

    func onSelectNode(_ url: String) {
        start(.select(url))
    }

    func onSelectNodeForDeletion(_ node: BsvHomeNodeSelection) {
        nodeDelete = node
    }

    func onPresentImportNode() {
        guard !isLoading, presentedImportNode == nil else { return }
        presentedImportNode = AddNodeSceneViewModel(chain: chain, service: service)
    }

    func onDismissImportNode(id: UUID) {
        guard presentedImportNode?.id == id else { return }
        presentedImportNode = nil
        start(.reload)
    }

    func onDeleteNode() {
        guard let nodeDelete else { return }
        start(.delete(nodeDelete))
    }

    func onRetry() {
        guard canRetry, let retryAction else { return }
        switch retryAction {
        case let .explorer(name): onSelectExplorer(name: name)
        case let .nodes(action): start(action)
        }
    }
}

extension ChainSettingsSceneViewModel {
    @discardableResult
    private func start(_ action: NodeAction) -> Task<Void, Never>? {
        guard !isLoading else { return nil }
        isLoading = true
        errorMessage = nil
        retryAction = nil
        let task = Task {
            defer {
                isLoading = false
                operationTask = nil
            }
            var retry = action
            do {
                try Task.checkCancellation()
                switch action {
                case .reload: break
                case let .select(url):
                    try await service.selectNode(chain: chain.rawValue, url: url)
                    clearNodeSelections()
                    retry = .reload
                case let .delete(node):
                    try await service.deleteNode(chain: chain.rawValue, url: node.url)
                    clearNodeSelections()
                    nodeDelete = nil
                    retry = .reload
                }
                try Task.checkCancellation()
                try await loadNodes()
                await loadNodesStates()
            } catch is CancellationError {
            } catch BsvHomeServiceError.Cancelled {
            } catch {
                switch action {
                case .reload: break
                case .select, .delete: clearNodeSelections()
                }
                errorMessage = Localized.Errors.errorOccurred
                retryAction = .nodes(retry)
            }
        }
        operationTask = task
        return task
    }

    private func loadNodes() async throws {
        let loaded = try await service.nodes(chain: chain.rawValue)
        try Task.checkCancellation()
        nodes = loaded
        statusStateByNodeUrl = [:]
    }

    private func clearNodeSelections() {
        nodes = nodes.map { node in
            var value = node
            value.isSelected = false
            return value
        }
    }

    private func loadNodesStates() async {
        await withTaskGroup(of: (String, BsvHomeNodeStatusState).self) { group in
            for url in nodes.map(\.url) {
                group.addTask {
                    await (url, self.service.nodeStatus(chain: self.chain.rawValue, url: url))
                }
            }

            for await (url, state) in group {
                guard !Task.isCancelled else { continue }
                statusStateByNodeUrl[url] = state
            }
        }
    }
}
