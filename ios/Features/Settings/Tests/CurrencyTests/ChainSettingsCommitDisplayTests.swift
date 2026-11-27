import BsvHomeCore
import enum Primitives.Chain
@testable import Settings
import Testing

@MainActor
struct ChainSettingsCommitDisplayTests {
    @Test
    func aRejectedExplorerWriteKeepsTheDisplayedCommittedChoice() {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        service.rejectExplorerWrites = true
        model.onSelectExplorer(name: "Other Explorer")
        #expect(service.explorerName(chain: "bitcoinsv") == "Committed Explorer")
        #expect(model.selectedExplorer == service.explorerName(chain: "bitcoinsv"))
    }

    @Test
    func aSuccessfulExplorerWriteDisplaysTheCommittedPreference() {
        let service = ChainSettingsServiceMock()
        let model = ChainSettingsSceneViewModel(chain: .bitcoinSv, service: service)
        model.onSelectExplorer(name: "Other Explorer")
        #expect(service.explorerName(chain: "bitcoinsv") == "Other Explorer")
        #expect(model.selectedExplorer == service.explorerName(chain: "bitcoinsv"))
    }
}
