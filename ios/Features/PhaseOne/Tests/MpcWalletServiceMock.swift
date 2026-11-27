import struct BsvHomeCore.BsvHomeMpcBackupMaterial
import struct BsvHomeCore.BsvHomeMpcPendingPreparation
import struct BsvHomeCore.BsvHomeMpcPendingTask
import struct BsvHomeCore.BsvHomeMpcTransfer
import struct BsvHomeCore.BsvHomeMpcTransferReview
import struct BsvHomeCore.BsvHomeMpcWallet
import class BsvHomeCore.BsvHomeMpcWalletService

final class MpcWalletServiceMock: BsvHomeMpcWalletService, @unchecked Sendable {
    var prepareReply: @Sendable () async throws -> BsvHomeMpcTransfer = { MpcWalletServiceMock.transfer }
    let calls = MpcWalletMockCalls()

    override func localWallets() throws -> [BsvHomeMpcWallet] {
        Self.wallets
    }

    override func listWallets() async throws -> [BsvHomeMpcWallet] {
        Self.wallets
    }

    override func pendingPreparation() throws -> BsvHomeMpcPendingPreparation? {
        nil
    }

    override func pendingTask() throws -> BsvHomeMpcPendingTask? {
        nil
    }

    override func pendingTransfers(walletId _: String) throws -> [BsvHomeMpcTransfer] {
        []
    }

    override func prepareTransferBsv(walletId _: String, recipient _: String, amountBsv _: String, idempotencyKey _: String) async throws -> BsvHomeMpcTransfer {
        await calls.prepare()
        return try await prepareReply()
    }

    override func signTransfer(id _: String, confirmedCommitment _: String) async throws -> BsvHomeMpcTransfer {
        await calls.sign(); return Self.transfer
    }

    override func broadcastTransfer(id _: String, confirmedCommitment _: String) async throws -> BsvHomeMpcTransfer {
        await calls.broadcast(); return Self.transfer
    }

    static var wallets: [BsvHomeMpcWallet] {
        ["wallet-a", "wallet-b"].map { BsvHomeMpcWallet(walletId: $0, name: $0, publicKey: "public-fixture", address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", epoch: 0, status: "ACTIVE", restored: false, balance: nil, actions: [.exportBackup]) }
    }

    static var transfer: BsvHomeMpcTransfer {
        BsvHomeMpcTransfer(id: "original-transfer", walletId: "wallet-a", recipient: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", recipientHandle: nil, amount: 1, fee: 2, status: "PREPARED", transactionId: nil, confirmations: nil, review: BsvHomeMpcTransferReview(recipient: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", recipientHandle: nil, amount: 1, fee: 2, change: 3, digests: [], unsignedTransaction: "public-fixture", commitment: "review-commitment"), actions: [.sign], errorCode: nil)
    }

    static func material(walletId: String = "wallet-a", epoch: UInt64 = 0, commitment: String = "file-commitment") -> BsvHomeMpcBackupMaterial {
        BsvHomeMpcBackupMaterial(walletId: walletId, ownerId: "test-user", environment: "https://test.example/v2/api/", epoch: epoch, document: #"{"context":{"wallet_id":"public-fixture"},"nonce":"fixture","ciphertext":"encrypted-fixture"}"#, recoveryKey: String(repeating: "a", count: 64), commitment: commitment)
    }
}
