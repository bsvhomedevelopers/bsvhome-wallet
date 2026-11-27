import BsvHomeCore
import Localization
@testable import Settings
import Testing

@MainActor
struct ChainSettingsFailureRetryTests {
    @Test
    func anInitialReadFailureShowsALocalizedErrorAndRetriesTheActualRead() async throws {
        let service = ChainSettingsServiceMock()
        service.rejectNodeReads = true
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        #expect(model.nodesModels.isEmpty)
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        #expect(model.canRetry)
        #expect(!model.isLoading)
        service.rejectNodeReads = false
        model.onRetry()
        try await (#require(model.operationTask)).value
        #expect(model.nodesModels.count == 2)
        #expect(model.errorMessage == nil)
        #expect(!model.canRetry)
        #expect(service.nodeReadCount == 2)
    }

    @Test
    func explorerRetryPersistsThePreviouslyRejectedChoiceBeforeDisplayingIt() {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        service.rejectExplorerWrites = true
        model.onSelectExplorer(name: "Other Explorer")
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        #expect(model.canRetry)
        #expect(model.selectedExplorer == "Committed Explorer")
        service.rejectExplorerWrites = false
        model.onRetry()
        #expect(model.selectedExplorer == "Other Explorer")
        #expect(model.selectedExplorer == service.explorerName(chain: "bitcoinsv"))
        #expect(model.errorMessage == nil)
        #expect(!model.canRetry)
    }

    @Test
    func aRejectedNodeSelectionHasNoUnverifiedCheckAndRetriesTheExplicitChoice() async throws {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        let target = try #require(service.committedNodes.last)
        service.rejectNodeSelections = true
        model.onSelectNode(target.url)
        try await (#require(model.operationTask)).value
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        #expect(model.nodesModels.allSatisfy { $0.selection == nil })
        #expect(service.committedNodes.first?.isSelected == true)
        service.rejectNodeSelections = false
        model.onRetry()
        try await (#require(model.operationTask)).value
        #expect(service.nodeSelectionCount == 2)
        #expect(model.nodesModels.first(where: { $0.url == target.url })?.selection == target.url)
        #expect(model.errorMessage == nil)
    }

    @Test
    func aCommittedSelectionWithAFailedReadRetriesOnlyTheSnapshot() async throws {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        let target = try #require(service.committedNodes.last)
        service.failReadsAfterMutation = true
        model.onSelectNode(target.url)
        try await (#require(model.operationTask)).value
        #expect(service.committedNodes.last?.isSelected == true)
        #expect(model.nodesModels.allSatisfy { $0.selection == nil })
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        service.rejectNodeReads = false
        model.onRetry()
        try await (#require(model.operationTask)).value
        #expect(service.nodeSelectionCount == 1)
        #expect(model.nodesModels.first(where: { $0.url == target.url })?.selection == target.url)
        #expect(model.errorMessage == nil)
    }

    @Test
    func aRejectedDeletionRemainsVisibleAndCanRetryTheSameNode() async throws {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        let target = try #require(service.committedNodes.last)
        service.rejectNodeDeletions = true
        model.onSelectNodeForDeletion(target)
        model.onDeleteNode()
        try await (#require(model.operationTask)).value
        #expect(model.nodesModels.contains { $0.url == target.url })
        #expect(model.nodesModels.allSatisfy { $0.selection == nil })
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        service.rejectNodeDeletions = false
        model.onRetry()
        try await (#require(model.operationTask)).value
        #expect(service.nodeDeletionCount == 2)
        #expect(!model.nodesModels.contains { $0.url == target.url })
        #expect(model.nodeDelete == nil)
        #expect(model.errorMessage == nil)
    }

    @Test
    func aDeletionFailureAfterTheStoredSelectionChangedCannotShowTheOldCheck() async throws {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        let target = try #require(service.committedNodes.last)
        model.onSelectNode(target.url)
        try await (#require(model.operationTask)).value
        try #require(model.nodesModels.last?.selection == target.url)
        service.rejectNodeDeletions = true
        service.resetSelectionBeforeDeleteFailure = true
        model.onSelectNodeForDeletion(target)
        model.onDeleteNode()
        try await (#require(model.operationTask)).value
        #expect(service.committedNodes.first?.isSelected == true)
        #expect(service.committedNodes.last?.isSelected == false)
        #expect(model.nodesModels.allSatisfy { $0.selection == nil })
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
    }

    @Test
    func aCommittedDeletionWithAFailedReadDoesNotDeleteAgainOnRetry() async throws {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        let target = try #require(service.committedNodes.last)
        service.failReadsAfterMutation = true
        model.onSelectNodeForDeletion(target)
        model.onDeleteNode()
        try await (#require(model.operationTask)).value
        #expect(!service.committedNodes.contains { $0.url == target.url })
        #expect(model.nodesModels.allSatisfy { $0.selection == nil })
        #expect(model.errorMessage == Localized.Errors.errorOccurred)
        service.rejectNodeReads = false
        model.onRetry()
        try await (#require(model.operationTask)).value
        #expect(service.nodeDeletionCount == 1)
        #expect(!model.nodesModels.contains { $0.url == target.url })
        #expect(model.errorMessage == nil)
    }

    @Test
    func aReloadFailureRetainsTheLastCommittedSnapshotAndBlocksDuplicateWrites() async throws {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        await model.load()
        let previous = model.nodesModels.map(\.node)
        service.rejectNodeReads = true
        await model.load()
        #expect(model.nodesModels.map(\.node) == previous)
        service.rejectNodeReads = false
        let target = try #require(service.committedNodes.last)
        model.onSelectNode(target.url)
        let task = try #require(model.operationTask)
        model.onSelectNode(target.url)
        model.onSelectExplorer(name: "Other Explorer")
        await task.value
        #expect(service.nodeSelectionCount == 1)
        #expect(service.explorerName(chain: "bitcoinsv") == "Committed Explorer")
        #expect(model.errorMessage == nil)
    }
}
