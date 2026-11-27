@testable import BsvColdWallet
import BsvHomeCore
import Testing

@MainActor
struct BsvColdWalletReceiveSelectionTests {
    @Test
    func noCurrentAddressKeepsThePendingAndIssuedRowsUntilAnExistingAddressIsSelected() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        var initial = BsvColdWalletServiceMock.observerFixture
        initial.addresses = .init(addresses: [
            .init(branch: .receive, index: 0, address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA", status: .used, isCurrent: false, isMain: true),
            .init(branch: .receive, index: 1, address: "1BitcoinEaterAddressDontSendf59kuE", status: .used, isCurrent: false, isMain: false),
        ], selectedIndex: nil, canAdd: false)
        initial.pending = BsvColdWalletServiceMock.signed
        let snapshot = initial
        service.snapshotReply = { snapshot }
        service.candidatesReply = { [.init(wallet: snapshot.wallet, role: .observer)] }
        let calls = Selections()
        var selected = snapshot.addresses
        selected.selectedIndex = 1
        selected.addresses[1].isCurrent = true
        let reply = selected
        service.selectAddressReply = { walletId, index in
            await calls.record(walletId: walletId, index: index)
            return reply
        }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: snapshot.wallet.id)
        await model.load()
        #expect(!model.loadFailed)
        #expect(model.errorMessage == nil)
        #expect(model.receiveAddress.isEmpty)
        #expect(model.addresses?.addresses == snapshot.addresses.addresses)
        #expect(model.pending == snapshot.pending)
        model.open(.receive)
        #expect(model.path == [.receive])
        model.selectAddress(1)
        let task = try #require(model.operationTask)
        await task.value
        #expect(await calls.walletIds == [snapshot.wallet.id])
        #expect(await calls.indices == [1])
        #expect(model.receiveAddress == reply.addresses[1].address)
        #expect(model.addresses == reply)
        #expect(model.pending == snapshot.pending)
        #expect(model.snapshot?.pairingJson == snapshot.pairingJson)
        #expect(model.path == [.receive])
    }

    @Test
    func aFailedSelectionDoesNotInventACurrentAddressOrRemoveThePendingAndIssuedRows() async throws {
        let service = BsvColdWalletServiceMock(noHandle: .init())
        var initial = BsvColdWalletServiceMock.observerFixture
        initial.addresses = .init(addresses: [
            .init(branch: .receive, index: 0, address: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA", status: .used, isCurrent: false, isMain: true),
        ], selectedIndex: nil, canAdd: false)
        initial.pending = BsvColdWalletServiceMock.signed
        let snapshot = initial
        service.snapshotReply = { snapshot }
        service.candidatesReply = { [.init(wallet: snapshot.wallet, role: .observer)] }
        service.selectAddressReply = { _, _ in throw BsvHomeServiceError.Store(msg: "Public fixture selection failure") }
        let model = BsvColdWalletViewModel(service: service, mode: .unified, walletId: snapshot.wallet.id)
        await model.load()
        model.open(.receive)
        model.selectAddress(0)
        let task = try #require(model.operationTask)
        await task.value
        #expect(model.errorMessage != nil)
        #expect(model.receiveAddress.isEmpty)
        #expect(model.addresses == snapshot.addresses)
        #expect(model.pending == snapshot.pending)
        #expect(model.path == [.receive])
        #expect(!model.loadFailed)
    }

    private actor Selections {
        private(set) var walletIds: [String] = []
        private(set) var indices: [UInt32] = []
        func record(walletId: String, index: UInt32) {
            walletIds.append(walletId)
            indices.append(index)
        }
    }
}
