import Components
@testable import BsvHome
import enum Primitives.QRScanType
import Testing

@MainActor
struct BsvMultisigScannerPresentationTests {
    @Test(arguments: [QRScanType.universal, .address])
    func internalNavigationKeepsTheScannerUntilItsPrefixIsRemoved(scanType: QRScanType) throws {
        let navigation = NavigationPathState()
        navigation.append("form")
        let scanner = BsvMultisigScannerPresentation()
        var codes: [String] = []
        scanner.present(navigation: navigation, isActive: { true }, scanType: scanType, onCode: { codes.append($0) })
        let id = try #require(scanner.presentation?.id)
        #expect(scanner.presentation?.scanType == scanType)
        navigation.append("review")
        navigation.removeLast()
        #expect(scanner.presentation?.id == id)
        navigation.removeLast()
        #expect(scanner.presentation == nil)
        scanner.complete(id: id, value: "late-public-code")
        #expect(codes.isEmpty)
    }

    @Test
    func replacingTheSameDepthRouteClosesTheScanner() throws {
        let navigation = NavigationPathState()
        navigation.append("first-form")
        let scanner = BsvMultisigScannerPresentation()
        var completed = false
        scanner.present(navigation: navigation, isActive: { true }, scanType: .address, onCode: { _ in completed = true })
        let id = try #require(scanner.presentation?.id)
        navigation.setPath(["replacement-form"])
        #expect(scanner.presentation == nil)
        scanner.complete(id: id, value: "late-public-code")
        #expect(!completed)
    }

    @Test
    func anOldDismissalOrCodeCannotCloseTheReplacement() throws {
        let navigation = NavigationPathState()
        let scanner = BsvMultisigScannerPresentation()
        var firstCodes: [String] = []
        var nextCodes: [String] = []
        scanner.present(navigation: navigation, isActive: { true }, scanType: .universal, onCode: { firstCodes.append($0) })
        let firstId = try #require(scanner.presentation?.id)
        let oldBinding = scanner.binding
        scanner.present(navigation: navigation, isActive: { true }, scanType: .address, onCode: { nextCodes.append($0) })
        let nextId = try #require(scanner.presentation?.id)
        oldBinding.wrappedValue = nil
        scanner.complete(id: firstId, value: "old-public-code")
        #expect(scanner.presentation?.id == nextId)
        scanner.complete(id: nextId, value: "new-public-code")
        #expect(firstCodes.isEmpty)
        #expect(nextCodes == ["new-public-code"])
        #expect(scanner.presentation == nil)
    }

    @Test
    func completionRemovesTheOldOwnerBeforeCallingTheConsumer() throws {
        let navigation = NavigationPathState()
        navigation.append("first-form")
        let scanner = BsvMultisigScannerPresentation()
        var completionCount = 0
        scanner.present(navigation: navigation, isActive: { true }, scanType: .universal, onCode: { _ in
            #expect(scanner.presentation == nil)
            completionCount += 1
            navigation.setPath(["next-form"])
            scanner.present(navigation: navigation, isActive: { true }, scanType: .address, onCode: { _ in })
        })
        let firstId = try #require(scanner.presentation?.id)
        let oldBinding = scanner.binding
        scanner.complete(id: firstId, value: "public-code")
        let nextId = try #require(scanner.presentation?.id)
        #expect(firstId != nextId)
        oldBinding.wrappedValue = nil
        scanner.complete(id: firstId, value: "duplicate-public-code")
        #expect(scanner.presentation?.id == nextId)
        #expect(completionCount == 1)
        navigation.reset()
        #expect(scanner.presentation == nil)
    }

    @Test
    func aChangedAdmissionClosesTheScannerWithoutDeliveringTheCode() throws {
        let scanner = BsvMultisigScannerPresentation()
        let navigation = NavigationPathState()
        var active = true
        var completionCount = 0
        scanner.present(navigation: navigation, isActive: { active }, scanType: .address, onCode: { _ in completionCount += 1 })
        let id = try #require(scanner.presentation?.id)
        active = false
        scanner.complete(id: id, value: "late-public-code")
        #expect(scanner.presentation == nil)
        #expect(completionCount == 0)
        scanner.present(navigation: navigation, isActive: { active }, scanType: .address, onCode: { _ in completionCount += 1 })
        #expect(scanner.presentation == nil)
        #expect(completionCount == 0)
    }

    @Test
    func explicitCancellationInvalidatesThePendingCode() throws {
        let scanner = BsvMultisigScannerPresentation()
        let navigation = NavigationPathState()
        var codes: [String] = []
        scanner.present(navigation: navigation, isActive: { true }, scanType: .address, onCode: { codes.append($0) })
        let id = try #require(scanner.presentation?.id)
        scanner.cancel()
        scanner.complete(id: id, value: "late-public-code")
        #expect(scanner.presentation == nil)
        #expect(codes.isEmpty)
    }
}
