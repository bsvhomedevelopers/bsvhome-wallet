// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct BsvHomeCore.Chain
import enum BsvHomeCore.BsvHomeServiceError
import struct BsvHomeCore.BsvHomeWalletDefaultName
import enum BsvHomeCore.BsvHomeWalletImportKind
import enum BsvHomeCore.BsvHomeWalletImportResult
import enum BsvHomeCore.BsvHomeWalletImportType
import class BsvHomeCore.BsvHomeWalletService
import struct BsvHomeCore.NameRecord
import struct BsvHomeCore.Wallet
import enum BsvHomeCore.WalletSource
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import BsvHomeCoreServicesTestKit
import Primitives
import PrimitivesTestKit

final class MultisigIdentityWalletServiceMock: BsvHomeWalletService, @unchecked Sendable {
    private let base = BsvHomeWalletService.mock(environment: .wowPay)
    private let lock = NSLock()
    private var imports = 0
    private var attempts: [BsvHomeWalletImportResult] = []
    private var sources: [BsvHomeCore.WalletSource] = []
    private var selections: [String] = []

    var result = BsvHomeWalletImportResult.new(wallet: Primitives.Wallet.mock(name: "Fixture identity").map())
    var completionError: BsvHomeServiceError? = .Store(msg: "metadata write failed")
    var importCount: Int { lock.withLock { imports } }
    var completionResults: [BsvHomeWalletImportResult] { lock.withLock { attempts } }
    var completionSources: [BsvHomeCore.WalletSource] { lock.withLock { sources } }
    var selectedWalletIds: [String] { lock.withLock { selections } }
    @MainActor var onImport: (@MainActor () async throws -> Void)?

    override func createWallet() throws -> [String] {
        LocalKeystore.words
    }

    override func defaultWalletName(chain _: BsvHomeCore.Chain?) async throws -> BsvHomeWalletDefaultName {
        BsvHomeWalletDefaultName(name: "Fixture identity", hasExistingWallets: true)
    }

    override func importRequest(kind: BsvHomeWalletImportKind, chain: BsvHomeCore.Chain?, input: String, nameRecord: NameRecord?) throws -> BsvHomeWalletImportType {
        try base.importRequest(kind: kind, chain: chain, input: input, nameRecord: nameRecord)
    }

    override func importName(nameRecord _: NameRecord?, defaultName: String) -> String {
        defaultName
    }

    override func importWallet(name _: String, import _: BsvHomeWalletImportType, source _: BsvHomeCore.WalletSource) async throws -> BsvHomeWalletImportResult {
        let result = lock.withLock {
            imports += 1
            return self.result
        }
        try await onImport?()
        return result
    }

    override func completeMultisigIdentityImport(result: BsvHomeWalletImportResult, source: BsvHomeCore.WalletSource) throws -> BsvHomeCore.Wallet {
        try lock.withLock {
            attempts.append(result)
            sources.append(source)
            if let completionError {
                throw completionError
            }
            return switch result {
            case let .new(wallet), let .existing(wallet): wallet
            }
        }
    }

    override func setCurrentWalletId(walletId: String) throws {
        lock.withLock { selections.append(walletId) }
    }
}
