import Components
import Foundation
import class BsvHomeCore.BsvHomeNameService
import enum BsvHomeCore.BsvHomePaymailError
import class BsvHomeCore.BsvHomeSendPublicFundingPreparation
import BsvHomeCorePrimitives
import BsvHomeCorePrimitivesTestKit
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit
import SwiftUI
import Testing
@testable import Transfer

@MainActor
struct WowPaySendPresentationTests {
    @Test(arguments: [0, 1, 2])
    func actualPathRemovalCancelsTheQuoteSynchronously(removal: Int) async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let model = try await makeModel(fixture: fixture, preparation: preparation)
        let navigation = NavigationPathState()
        navigation.append("asset")
        let presentation = WowPaySendPresentation()
        presentation.present(id: UUID(), model: model, navigation: navigation)
        let original = navigation.path
        navigation.append("review")
        navigation.removeLast()
        #expect(presentation.model === model)
        #expect(navigation.path == original)
        let beforeExit = try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        #expect(beforeExit.amount == "0.00042")

        switch removal {
        case 0: navigation.removeLast()
        case 1: navigation.reset()
        default: navigation.setPath(["replacement"])
        }

        #expect(presentation.model == nil)
        #expect(!model.isConfirmEnabled)
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        }
        #expect(navigation.count == (removal == 1 ? 0 : 1))
    }

    @Test
    func anOldCompletionCannotDismissTheReplacementSend() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let firstPreparation = try await fixture.prepare(value: 42000)
        let first = try await makeModel(fixture: fixture, preparation: firstPreparation)
        let secondPreparation = try await fixture.prepare(value: 42000)
        let second = try await makeModel(fixture: fixture, preparation: secondPreparation)
        let navigation = NavigationPathState()
        let presentation = WowPaySendPresentation()
        let oldId = UUID()
        let currentId = UUID()
        presentation.present(id: oldId, model: first, navigation: navigation)
        presentation.present(id: currentId, model: second, navigation: navigation)
        presentation.dismiss(id: oldId)

        #expect(presentation.model === second)
        #expect(presentation.isCurrent(currentId))
        #expect(navigation.count == 1)
        #expect(!first.isConfirmEnabled)
        let current = try await fixture.service.confirmPublicFunding(preparation: secondPreparation, value: 42000)
        #expect(current.amount == "0.00042")
        presentation.dismiss(id: currentId)
        #expect(navigation.isEmpty)
    }

    @Test
    func retirementCleanupRunsOnceBeforeAnExternalPathReplacementCommits() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let preparation = try await fixture.prepare(value: 42000)
        let model = try await makeModel(fixture: fixture, preparation: preparation)
        let navigation = NavigationPathState()
        navigation.append("asset")
        let presentation = WowPaySendPresentation()
        let id = UUID()
        var retirements = 0
        var retirementDepths: [Int] = []
        presentation.present(id: id, model: model, navigation: navigation, onRetire: {
            retirements += 1
            retirementDepths.append(navigation.count)
            #expect(presentation.id == nil)
            #expect(presentation.model == nil)
            #expect(!model.isConfirmEnabled)
            presentation.dismiss(id: id)
        })
        navigation.append("internal-page")
        navigation.removeLast()
        #expect(retirements == 0)
        navigation.reset()
        #expect(retirements == 1)
        #expect(retirementDepths == [2])
        #expect(navigation.isEmpty)
        presentation.dismiss(id: id)
        presentation.dismiss()
        #expect(retirements == 1)
        #expect(navigation.isEmpty)
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: preparation, value: 42000)
        }
        #expect(fixture.credentialAccessCount == 0)
    }

    @Test
    func anOldRetirementCallbackCannotDismissOrRunForTheReplacement() async throws {
        let fixture = try await SendPublicPaymailFixture.make()
        let oldPreparation = try await fixture.prepare(value: 42000)
        let oldModel = try await makeModel(fixture: fixture, preparation: oldPreparation)
        let nextPreparation = try await fixture.prepare(value: 42000)
        let nextModel = try await makeModel(fixture: fixture, preparation: nextPreparation)
        let navigation = NavigationPathState()
        navigation.append("asset")
        let presentation = WowPaySendPresentation()
        let oldId = UUID()
        let nextId = UUID()
        var oldRetirements = 0
        presentation.present(id: oldId, model: oldModel, navigation: navigation, onRetire: {
            oldRetirements += 1
            #expect(presentation.id == nil)
            #expect(presentation.model == nil)
            presentation.dismiss(id: oldId)
        })
        presentation.present(id: nextId, model: nextModel, navigation: navigation)
        #expect(oldRetirements == 1)
        presentation.dismiss(id: oldId)
        #expect(presentation.model === nextModel)
        #expect(presentation.isCurrent(nextId))
        #expect(navigation.count == 2)
        let current = try await fixture.service.confirmPublicFunding(preparation: nextPreparation, value: 42000)
        #expect(current.amount == "0.00042")
        presentation.dismiss(id: nextId)
        #expect(oldRetirements == 1)
        #expect(navigation.count == 1)
        await #expect(throws: BsvHomePaymailError.Auth(error: .Service(error: .Cancelled))) {
            try await fixture.service.confirmPublicFunding(preparation: nextPreparation, value: 42000)
        }
        #expect(fixture.credentialAccessCount == 0)
    }

    private func makeModel(fixture: SendPublicPaymailFixture, preparation: BsvHomeSendPublicFundingPreparation) async throws -> WowPaySendViewModel {
        let wallet = fixture.wallet.map()
        let account = try #require(wallet.accounts.first)
        let service = BsvHomeSendServiceMock()
        service.onResolveDestination = { _, _, _, _ in .publicFunding(preparation: preparation) }
        let model = WowPaySendViewModel(
            wallet: wallet,
            assetData: .mock(asset: fixture.asset.map(), balance: .mock(available: 2_000_000), account: account),
            service: service,
            nameService: BsvHomeNameService.mock(),
            prepareBsvAddresses: {},
            feeLoader: { _ in
                Issue.record("Public funding entered ordinary fee loading")
                return .mock()
            },
            onTransferAction: nil,
            onPublicFunding: { _ in },
        )
        await model.prepareBsvAddresses()
        model.addressInputModel.update(text: "receiver@example.invalid")
        model.onChangeAddressText("", new: "receiver@example.invalid")
        model.amountInputModel.text = "0.00042"
        model.onChangeAmountText("", "0.00042")
        try await (#require(model.feeTask)).value
        #expect(model.isConfirmEnabled)
        return model
    }
}
