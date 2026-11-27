import Foundation
import protocol BsvHomeCore.BsvHomeBsvAddressStore
import struct BsvHomeCore.BsvHomeBsvAddressStoredState
import struct BsvHomeCore.BsvHomeBsvFundingStoredState
import typealias BsvHomeCore.Transaction
import BsvHomeCorePrimitives
import Primitives
import Store

public final class BsvHomeCoreBsvAddressStore: BsvHomeBsvAddressStore, @unchecked Sendable {
    private let store: BsvAddressPoolStore
    private let fundingDirectory: URL?

    public init(store: BsvAddressPoolStore, fundingDirectory: URL? = nil) {
        self.store = store
        self.fundingDirectory = fundingDirectory
    }

    public func fundingStates() async throws -> [BsvHomeBsvFundingStoredState] {
        try BsvHomeCoreBsvMultisigStore.fundingStates(directory: fundingDirectory)
    }

    public func get(walletId: String) throws -> BsvHomeBsvAddressStoredState? {
        try store.get(walletId: WalletId.from(id: walletId)).map {
            BsvHomeBsvAddressStoredState(revision: $0.revision, value: $0.value)
        }
    }

    public func save(walletId: String, expectedRevision: UInt64?, state: BsvHomeBsvAddressStoredState) async throws -> Bool {
        try store.save(walletId: WalletId.from(id: walletId), expectedRevision: expectedRevision, revision: state.revision, value: state.value)
    }

    public func unresolvedTransactions(walletId: String) throws -> [BsvHomeCore.Transaction] {
        try store.unresolvedTransactions(walletId: WalletId.from(id: walletId)).map { $0.map() }
    }
}
