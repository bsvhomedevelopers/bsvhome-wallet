import Foundation
import enum BsvHomeCore.BsvHomeMpcError
import struct BsvHomeCore.BsvHomePaymentRecipient
import enum BsvHomeCore.BsvHomePlatformAccountError
import struct BsvHomeCore.BsvHomeRecipient
@testable import PhaseOne
import Testing

@MainActor
struct MpcWalletSafetyTests {
    @Test
    func switchingWalletRejectsLatePreparationAndOldPaymentAuthorization() async {
        let service = MpcWalletServiceMock(noHandle: .init())
        let gate = MpcTransferGate()
        service.prepareReply = { await gate.response() }
        var suspensions = 0
        let model = MpcWalletViewModel(service: service, authenticate: {}, onWalletChange: { suspensions += 1 })
        model.selectWallet("wallet-a")
        model.recipient = "public-recipient"
        model.amount = "0.00000001"
        let pending = Task { await model.prepare("wallet-a") }
        await gate.waitUntilRequested()
        model.selectWallet("wallet-b")
        await gate.finish(MpcWalletServiceMock.transfer)
        #expect(await pending.value == nil)
        #expect(model.transfer == nil)
        #expect(model.recipient.isEmpty)
        #expect(model.amount.isEmpty)
        #expect(suspensions == 1)
        model.transfer = MpcWalletServiceMock.transfer
        await model.signTransfer()
        await model.broadcastTransfer()
        #expect(await service.calls.signatures == 0)
        #expect(await service.calls.broadcasts == 0)
    }

    @Test
    func originalExportReceiptCannotMarkAnotherWalletOrRegeneratedEpochAsSaved() async throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        model.backup = MpcWalletServiceMock.material()
        let original = try #require(await model.makeBackupExport())
        defer { try? original.removeTemporaryFile() }
        #expect(try Data(contentsOf: original.fileURL) == Data(MpcWalletServiceMock.material().document.utf8))
        #expect(try original.fileURL.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup == true)
        model.selectWallet("wallet-b")
        model.backup = MpcWalletServiceMock.material(walletId: "wallet-b", epoch: 1, commitment: "new-file")
        let current = try #require(await model.makeBackupExport())
        defer { try? current.removeTemporaryFile() }
        model.backupExportFinished(original, outcome: .saved, cleanupFailed: false)
        #expect(model.savedCommitment == nil)
        model.backupExportFinished(original, outcome: .abandoned, cleanupFailed: false)
        #expect(model.backup?.walletId == "wallet-b")
        model.backupExportFinished(current, outcome: .saved, cleanupFailed: false)
        #expect(model.savedCommitment == "new-file")
        model.backup = MpcWalletServiceMock.material(walletId: "wallet-b", epoch: 2, commitment: "next-epoch")
        model.savedCommitment = nil
        model.backupExportFinished(current, outcome: .saved, cleanupFailed: false)
        #expect(model.savedCommitment == nil)
    }

    @Test
    func oldBackupPageTeardownCannotClearANewTicketForTheSameWallet() async throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        let previousOwner = UUID()
        model.enterBackupPage(previousOwner)
        model.backup = MpcWalletServiceMock.material()
        let previous = try #require(await model.makeBackupExport())
        defer { try? previous.removeTemporaryFile() }
        let gate = MpcTransferGate()
        let late = Task { await model.value { _ in await gate.response() } }
        await gate.waitUntilRequested()
        let currentOwner = UUID()
        model.enterBackupPage(currentOwner)
        await gate.finish(MpcWalletServiceMock.transfer)
        #expect(await late.value == nil)
        model.backup = MpcWalletServiceMock.material(epoch: 1, commitment: "new-epoch")
        let current = try #require(await model.makeBackupExport())
        defer { try? current.removeTemporaryFile() }
        #expect(!model.leaveBackupPage("wallet-a", owner: previousOwner))
        #expect(model.backup?.commitment == "new-epoch")
        #expect(model.isCurrentExport(current))
        #expect(FileManager.default.fileExists(atPath: current.fileURL.path))
        #expect(model.leaveBackupPage("wallet-a", owner: currentOwner))
        #expect(model.backup == nil)
    }

    @Test
    func pickerCancellationCleansItsOwnFileAndCannotBecomeASuccessfulReceipt() async throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        model.backup = MpcWalletServiceMock.material()
        let ticket = try #require(await model.makeBackupExport())
        let coordinator = MpcBackupDocumentPicker.Coordinator(ticket: ticket) { ticket, outcome, failed in
            model.backupExportFinished(ticket, outcome: outcome, cleanupFailed: failed)
        }
        coordinator.finish(saved: false)
        coordinator.finish(saved: true)
        #expect(model.savedCommitment == nil)
        #expect(!FileManager.default.fileExists(atPath: ticket.fileURL.path))
        #expect(model.backup != nil)
    }

    @Test
    func abandonedPickerClearsTheOriginalMaterialAndCannotLaterSaveItsReceipt() async throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        model.backup = MpcWalletServiceMock.material()
        model.recoveryKeyConfirmation = "synthetic fixture confirmation"
        let ticket = try #require(await model.makeBackupExport())
        let coordinator = MpcBackupDocumentPicker.Coordinator(ticket: ticket) { ticket, outcome, failed in
            model.backupExportFinished(ticket, outcome: outcome, cleanupFailed: failed)
        }
        coordinator.abandon()
        coordinator.finish(saved: true)
        #expect(model.backup == nil)
        #expect(model.recoveryKeyConfirmation.isEmpty)
        #expect(model.savedCommitment == nil)
        #expect(!FileManager.default.fileExists(atPath: ticket.fileURL.path))
    }

    @Test
    func backgroundInvalidationRejectsAReceiptAfterReturningToTheSameWallet() async throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        model.backup = MpcWalletServiceMock.material()
        let ticket = try #require(await model.makeBackupExport())
        defer { try? ticket.removeTemporaryFile() }
        model.clearSensitive()
        model.backup = MpcWalletServiceMock.material()
        model.backupExportFinished(ticket, outcome: .saved, cleanupFailed: false)
        #expect(model.savedCommitment == nil)
    }

    @Test
    func failedWalletCloseBlocksCoreWorkUntilAnExplicitRetrySucceeds() async {
        let service = MpcWalletServiceMock(noHandle: .init())
        service.prepareReply = {
            var transfer = MpcWalletServiceMock.transfer
            transfer.walletId = "wallet-b"
            return transfer
        }
        var attempts = 0
        let model = MpcWalletViewModel(service: service, authenticate: {}, onWalletChange: {
            attempts += 1
            if attempts == 1 {
                throw BsvHomeMpcError.NativeUnavailable
            }
        })
        model.selectWallet("wallet-a")
        model.selectWallet("wallet-b")
        model.recipient = "public-recipient"
        model.amount = "1"
        #expect(await model.prepare("wallet-b") == nil)
        #expect(await service.calls.preparations == 0)
        #expect(model.error != nil)
        #expect(await model.prepare("wallet-b") == "original-transfer")
        #expect(await service.calls.preparations == 1)
        #expect(attempts == 2)
    }

    @Test
    func typedSessionFailuresClearSensitiveInputsAndRefreshTheCoreSessionProjection() async {
        for failure in [BsvHomePlatformAccountError.LoginRequired, .SessionChanged] {
            let service = MpcWalletServiceMock(noHandle: .init())
            service.prepareReply = { throw BsvHomeMpcError.Platform(error: failure) }
            var reloads = 0
            let model = MpcWalletViewModel(service: service, authenticate: {}, onWalletChange: {}, onSessionInvalidated: { reloads += 1 })
            model.selectWallet("wallet-a")
            model.backup = MpcWalletServiceMock.material()
            model.importRecoveryKey = "synthetic sensitive input"
            #expect(await model.prepare("wallet-a") == nil)
            #expect(model.backup == nil)
            #expect(model.importRecoveryKey.isEmpty)
            #expect(reloads == 1)
        }
    }

    @Test
    func aLateSessionFailureRefreshesProjectionWithoutClearingANewerWalletBackup() async {
        let service = MpcWalletServiceMock(noHandle: .init())
        let gate = MpcTransferGate()
        service.prepareReply = {
            _ = await gate.response()
            throw BsvHomeMpcError.Platform(error: .LoginRequired)
        }
        var reloads = 0
        let model = MpcWalletViewModel(service: service, authenticate: {}, onWalletChange: {}, onSessionInvalidated: { reloads += 1 })
        model.selectWallet("wallet-a")
        let pending = Task { await model.prepare("wallet-a") }
        await gate.waitUntilRequested()
        model.selectWallet("wallet-b")
        model.backup = MpcWalletServiceMock.material(walletId: "wallet-b")
        await gate.finish(MpcWalletServiceMock.transfer)
        #expect(await pending.value == nil)
        #expect(reloads == 1)
        #expect(model.backup?.walletId == "wallet-b")
    }

    @Test
    func scannerAndContactCallbacksAreSingleUseAndBoundToTheOriginalWalletDraft() throws {
        let model = MpcWalletViewModel(service: MpcWalletServiceMock(noHandle: .init()), authenticate: {}, onWalletChange: {})
        model.selectWallet("wallet-a")
        let stale = try #require(model.recipientInputCallback(walletId: "wallet-a"))
        model.recipient = "new manual recipient"
        model.draftChanged()
        let payment = BsvHomePaymentRecipient(recipient: BsvHomeRecipient(address: "public-address", name: "Local contact"), amount: "0.00000001")
        #expect(!stale(payment))
        #expect(model.recipient == "new manual recipient")
        let current = try #require(model.recipientInputCallback(walletId: "wallet-a"))
        #expect(current(payment))
        #expect(model.recipient == "public-address")
        #expect(model.amount == "0.00000001")
        #expect(model.recipientName == "Local contact")
        #expect(model.transfer == nil)
        #expect(!current(payment))
        let otherWallet = try #require(model.recipientInputCallback(walletId: "wallet-a"))
        model.selectWallet("wallet-b")
        #expect(!otherWallet(payment))
        #expect(model.recipient.isEmpty)
        #expect(model.amount.isEmpty)
    }
}
