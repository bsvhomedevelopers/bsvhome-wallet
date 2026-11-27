public import BsvHomeCore
import Foundation
import BsvHomeCorePrimitives
import Primitives

public final class LocalKeystore: Keystore, @unchecked Sendable {
    public let bsvHomeKeystore: BsvHomeKeystore
    public let bsvColdWalletStore: any BsvHomeBsvColdWalletStore
    private let keystoreURL: URL
    private let keystorePassword: KeystorePassword
    private let queue = DispatchQueue(label: "com.bsvhome.keystore", qos: .userInitiated)

    private enum BsvMultisigKeyAccess {
        case publicIdentity
        case onlineAuthentication
    }

    public convenience init(
        directory: String = "keystore",
        keystorePassword: KeystorePassword = LocalKeystorePassword(),
        environment: BsvHomeRuntimeEnvironment = .production,
        coldWalletStore: (any BsvHomeBsvColdWalletStore)? = nil,
    ) {
        do {
            // migrate keystore from documents directory to application support directory
            // TODO: delete in 2026
            let fileMigrator = FileMigrator()
            let keystoreURL = try fileMigrator.migrate(
                name: directory,
                fromDirectory: .documentDirectory,
                toDirectory: .applicationSupportDirectory,
                isDirectory: true,
            )
            try self.init(directoryURL: keystoreURL, keystorePassword: keystorePassword, environment: environment, coldWalletStore: coldWalletStore)
        } catch {
            fatalError("keystore initialization error: \(error)")
        }
    }

    public init(
        directoryURL: URL,
        keystorePassword: KeystorePassword,
        environment: BsvHomeRuntimeEnvironment,
        coldWalletStore: (any BsvHomeBsvColdWalletStore)? = nil,
    ) throws {
        keystoreURL = directoryURL
        bsvHomeKeystore = try BsvHomeKeystore.newWithEnvironment(baseDir: directoryURL.path, environment: environment)
        self.keystorePassword = keystorePassword
        bsvColdWalletStore = try coldWalletStore ?? BsvHomeCoreBsvColdWalletStore()
    }

    public func keystorePassword(createIfMissing: Bool) throws -> String {
        let password = try keystorePassword.getPassword()
        if password.isNotEmpty {
            return password
        }
        guard createIfMissing else {
            throw KeystoreError.missingPassword
        }
        let newPassword = try SecureRandom.generateKey(length: 32).hex
        try keystorePassword.setPassword(newPassword, authentication: .none)
        return newPassword
    }

    public func migrateV3Keystores(for wallets: [Primitives.Wallet]) async throws -> [KeystoreMigrationFailure] {
        let pendingWallets = pendingV3Migrations(for: wallets)
        guard pendingWallets.isNotEmpty else {
            return []
        }
        let password = try await getPassword()
        guard !password.isEmpty else {
            return pendingWallets.map {
                KeystoreMigrationFailure(walletId: $0.wallet.id, error: AnyError("keystore password is missing"))
            }
        }

        var failures: [KeystoreMigrationFailure] = []
        for pending in pendingWallets {
            do {
                try await migrateV3Keystore(wallet: pending.wallet, v3URL: pending.v3URL, password: password)
            } catch {
                failures.append(KeystoreMigrationFailure(walletId: pending.wallet.id, error: error))
            }
        }
        return failures
    }

    public func sign(wallet: Primitives.Wallet, input: BsvHomeSignerInput) async throws -> [BsvHomeSignedTransaction] {
        let chain = input.input.chain()
        try requireHotSigning(wallet: wallet, chain: chain)
        let password = try await getPassword(wallet: wallet, chain: chain)
        try requireHotSigning(wallet: wallet, chain: chain)
        let keystoreId = bsvHomeKeystore.keystoreId(walletId: wallet.id.id)
        return try await hotSigningOperation(wallet: wallet, chain: chain) { [self, bsvHomeKeystore] in
            try withV4Password(keystore: bsvHomeKeystore, password) { passwordBytes in
                try requireHotSigning(wallet: wallet, chain: chain)
                return try bsvHomeKeystore.sign(keystoreId: keystoreId, chain: chain, input: input, password: passwordBytes)
            }
        }
    }

    public func signBsvColdRequest(wallet: BsvHomeCore.Wallet, requestJSON: String, reviewJSON: String, scope: BsvHomeBsvColdWalletActionScope) async throws -> String {
        try Task.checkCancellation()
        try scope.checkSigning(wallet: wallet, requestJson: requestJSON, reviewJson: reviewJSON)
        let password = try await getPassword(scope: scope)
        try await scope.validate()
        try Task.checkCancellation()
        let identifier = bsvHomeKeystore.keystoreId(walletId: wallet.id)
        return try await bsvColdOperation(wallet: wallet, requestJSON: requestJSON, reviewJSON: reviewJSON, scope: scope) { [bsvHomeKeystore] in
            try withV4Password(keystore: bsvHomeKeystore, password) { passwordBytes in
                try scope.checkSigning(wallet: wallet, requestJson: requestJSON, reviewJson: reviewJSON)
                return try bsvHomeKeystore.bsvColdSign(keystoreId: identifier, requestJson: requestJSON, reviewJson: reviewJSON, password: passwordBytes)
            }
        }
    }

    func hotSigningOperation<T: Sendable>(wallet: Primitives.Wallet, chain: BsvHomeCore.Chain, onEnqueued: (@Sendable () -> Void)? = nil, operation: @escaping @Sendable () throws -> T) async throws -> T {
        try requireHotSigning(wallet: wallet, chain: chain)
        return try await queue.asyncTask(onEnqueued: onEnqueued) { [self] in
            try requireHotSigning(wallet: wallet, chain: chain)
            let result = try operation()
            try requireHotSigning(wallet: wallet, chain: chain)
            return result
        }
    }

    func bsvColdOperation<T: Sendable>(wallet: BsvHomeCore.Wallet, requestJSON: String, reviewJSON: String, scope: BsvHomeBsvColdWalletActionScope, onEnqueued: (@Sendable () -> Void)? = nil, operation: @escaping @Sendable () throws -> T) async throws -> T {
        try Task.checkCancellation()
        try scope.checkSigning(wallet: wallet, requestJson: requestJSON, reviewJson: reviewJSON)
        return try await withTaskCancellationHandler {
            try await queue.asyncTask(onEnqueued: onEnqueued) {
                try scope.checkSigning(wallet: wallet, requestJson: requestJSON, reviewJson: reviewJSON)
                let result = try operation()
                try scope.checkSigning(wallet: wallet, requestJson: requestJSON, reviewJson: reviewJSON)
                return result
            }
        } onCancel: {
            scope.cancel()
        }
    }

    private func requireHotSigning(wallet: Primitives.Wallet, chain: BsvHomeCore.Chain) throws {
        try bsvColdWalletRequireHotSigning(wallet: wallet.map(), chain: chain, store: bsvColdWalletStore)
    }

    public func signMessage(signer: MessageSigner, wallet: Primitives.Wallet) async throws -> String {
        let password = try await getPassword()
        let keystoreId = bsvHomeKeystore.keystoreId(walletId: wallet.id.id)
        return try await queue.asyncTask { [bsvHomeKeystore] in
            try withV4Password(keystore: bsvHomeKeystore, password) { passwordBytes in
                try signer.signWithKeystore(keystore: bsvHomeKeystore, keystoreId: keystoreId, password: passwordBytes)
            }
        }
    }

    public func bsvMultisigPublicKey(wallet: Primitives.Wallet, scope: BsvHomeBsvMultisigActionScope? = nil) async throws -> String {
        try await bsvMultisigPublicKey(wallet: wallet, scope: scope, access: .publicIdentity)
    }

    public func authenticateBsvMultisigWallet(wallet: Primitives.Wallet, scope: BsvHomeBsvMultisigActionScope?) async throws {
        _ = try await bsvMultisigPublicKey(wallet: wallet, scope: scope, access: .onlineAuthentication)
    }

    private func bsvMultisigPublicKey(wallet: Primitives.Wallet, scope: BsvHomeBsvMultisigActionScope?, access: BsvMultisigKeyAccess) async throws -> String {
        guard wallet.canSign, let account = wallet.accounts.first(where: { $0.chain == .bitcoinSv }) else {
            throw BsvHomeServiceError.InvalidInput(msg: "BSV signing wallet is unavailable")
        }
        try scopeCheck(scope)
        let password: String = if access == .onlineAuthentication {
            try await getPassword(wallet: wallet, scope: scope)
        } else {
            try await getPassword(scope: scope)
        }
        try scopeCheck(scope)
        let keystoreId = bsvHomeKeystore.keystoreId(walletId: wallet.id.id)
        let operation: @Sendable () throws -> String = { [self, bsvHomeKeystore] in
            try withV4Password(keystore: bsvHomeKeystore, password) { passwordBytes in
                try scopeCheck(scope)
                if access == .onlineAuthentication {
                    try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
                }
                let publicKey = try bsvHomeKeystore.bsvMultisigPublicKey(keystoreId: keystoreId, password: passwordBytes)
                guard try bsvHomeKeystore.bsvMultisigMemberAddress(publicKey: publicKey) == account.address else {
                    throw BsvHomeServiceError.InvalidInput(msg: "BSV signing identity does not match its account")
                }
                return publicKey
            }
        }
        if access == .onlineAuthentication {
            return try await bsvMultisigSigningOperation(wallet: wallet, scope: scope, operation: operation)
        }
        return try await bsvMultisigOperation(scope: scope, operation: operation)
    }

    public func signBsvMultisigProposal(wallet: Primitives.Wallet, policyJSON: String, proposalJSON: String, recipient: String, amount: UInt64, maximumFee: UInt64, scope: BsvHomeBsvMultisigActionScope) async throws -> String {
        guard wallet.canSign, wallet.accounts.contains(where: { $0.chain == .bitcoinSv }) else {
            throw BsvHomeServiceError.InvalidInput(msg: "BSV signing wallet is unavailable")
        }
        try scopeCheck(scope)
        let password = try await getPassword(wallet: wallet, scope: scope)
        try scopeCheck(scope)
        let keystoreId = bsvHomeKeystore.keystoreId(walletId: wallet.id.id)
        return try await bsvMultisigSigningOperation(wallet: wallet, scope: scope) { [self, bsvHomeKeystore] in
            try withV4Password(keystore: bsvHomeKeystore, password) { passwordBytes in
                try scopeCheck(scope)
                try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
                return try bsvHomeKeystore.bsvMultisigSign(keystoreId: keystoreId, policyJson: policyJSON, proposalJson: proposalJSON, expectedRecipient: recipient, expectedAmount: amount, maximumFee: maximumFee, password: passwordBytes)
            }
        }
    }

    public func signBsvMultisigFunding(wallet: Primitives.Wallet, fundingJSON: String, reviewJSON: String, scope: BsvHomeBsvMultisigActionScope) async throws -> String {
        guard wallet.canSign, wallet.accounts.contains(where: { $0.chain == .bitcoinSv }) else {
            throw BsvHomeServiceError.InvalidInput(msg: "BSV signing wallet is unavailable")
        }
        try scopeCheck(scope)
        let password = try await getPassword(wallet: wallet, scope: scope)
        try scopeCheck(scope)
        let keystoreId = bsvHomeKeystore.keystoreId(walletId: wallet.id.id)
        return try await bsvMultisigSigningOperation(wallet: wallet, scope: scope) { [self, bsvHomeKeystore] in
            try withV4Password(keystore: bsvHomeKeystore, password) { passwordBytes in
                try scopeCheck(scope)
                try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
                return try bsvHomeKeystore.bsvMultisigFundingSign(keystoreId: keystoreId, fundingJson: fundingJSON, reviewJson: reviewJSON, password: passwordBytes)
            }
        }
    }

    func bsvMultisigSigningOperation<T: Sendable>(wallet: Primitives.Wallet, scope: BsvHomeBsvMultisigActionScope?, onEnqueued: (@Sendable () -> Void)? = nil, operation: @escaping @Sendable () throws -> T) async throws -> T {
        try scopeCheck(scope)
        try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
        return try await bsvMultisigOperation(scope: scope, onEnqueued: onEnqueued) { [self] in
            try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
            let result = try operation()
            try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
            return result
        }
    }

    func bsvMultisigOperation<T: Sendable>(scope: BsvHomeBsvMultisigActionScope?, onEnqueued: (@Sendable () -> Void)? = nil, operation: @escaping @Sendable () throws -> T) async throws -> T {
        try await queue.asyncTask(onEnqueued: onEnqueued) {
            try scopeCheck(scope)
            let result = try operation()
            try scopeCheck(scope)
            return result
        }
    }

    public func getPasswordAuthentication() throws -> KeystoreAuthentication {
        try keystorePassword.getAuthentication()
    }

    public func applyFileProtection() throws {
        guard FileManager.default.fileExists(atPath: keystoreURL.path) else { return }
        try WalletFileProtection.apply(to: keystoreURL)
    }

    public func destroy() throws {
        guard FileManager.default.fileExists(atPath: keystoreURL.path) else {
            return
        }
        try FileManager.default.removeItem(at: keystoreURL)
    }

    @MainActor
    func getPassword() throws -> String {
        try keystorePassword.getPassword()
    }

    @MainActor
    func getPassword(scope: BsvHomeBsvMultisigActionScope?) throws -> String {
        try scopeCheck(scope)
        let password = try getPassword()
        try scopeCheck(scope)
        return password
    }

    @MainActor
    func getPassword(wallet: Primitives.Wallet, scope: BsvHomeBsvMultisigActionScope?) throws -> String {
        try scopeCheck(scope)
        try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
        let password = try getPassword()
        try requireHotSigning(wallet: wallet, chain: Primitives.Chain.bitcoinSv.rawValue)
        try scopeCheck(scope)
        return password
    }

    @MainActor
    func getPassword(wallet: Primitives.Wallet, chain: BsvHomeCore.Chain) throws -> String {
        try requireHotSigning(wallet: wallet, chain: chain)
        let password = try getPassword()
        try requireHotSigning(wallet: wallet, chain: chain)
        return password
    }

    @MainActor
    private func getPassword(scope: BsvHomeBsvColdWalletActionScope) throws -> String {
        try Task.checkCancellation()
        try scope.check()
        let password = try getPassword()
        try scope.check()
        try Task.checkCancellation()
        return password
    }

    private func pendingV3Migrations(for wallets: [Primitives.Wallet]) -> [(wallet: Primitives.Wallet, v3URL: URL)] {
        wallets.compactMap { wallet in
            switch wallet.type {
            case .view:
                return nil
            case .multicoin, .single, .privateKey:
                guard let v3URL = Self.findV3File(in: keystoreURL, matching: wallet.legacyV3Id) else {
                    return nil
                }
                return (wallet, v3URL)
            }
        }
    }

    private func migrateV3Keystore(wallet: Primitives.Wallet, v3URL: URL, password: String) async throws {
        try await queue.asyncTask { [bsvHomeKeystore] in
            var v3Password = password.v3PasswordBytes()
            var newPassword = bsvHomeKeystore.decodePassword(password: password)
            defer {
                v3Password.zeroize()
                newPassword.zeroize()
            }
            _ = try bsvHomeKeystore.migrateV3(
                v3Path: v3URL.path,
                v3Password: v3Password,
                newPassword: newPassword,
                walletId: wallet.id.id,
            )
        }
    }

    private static func findV3File(in directory: URL, matching keystoreId: String) -> URL? {
        let target = keystoreId.lowercased()
        let fileManager = FileManager.default
        guard let contents = try? fileManager.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.isDirectoryKey],
        ) else {
            return nil
        }
        for url in contents {
            let isDirectory = (try? url.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
            if isDirectory {
                continue
            }
            let name = url.lastPathComponent.lowercased()
            if name == target || name.hasSuffix(target) {
                return url
            }
            if let data = try? Data(contentsOf: url),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let fileId = (json["id"] as? String)?.lowercased(),
               fileId == target
            {
                return url
            }
        }
        return nil
    }
}

func withV4Password<T>(
    keystore: BsvHomeCore.BsvHomeKeystore,
    _ password: String,
    _ operation: (Data) throws -> T,
) throws -> T {
    guard password.isNotEmpty else {
        throw KeystoreError.missingPassword
    }
    var passwordBytes = keystore.decodePassword(password: password)
    defer { passwordBytes.zeroize() }
    return try operation(passwordBytes)
}

extension Primitives.Wallet {
    var legacyV3Id: String {
        externalId ?? id.id
    }
}

private func scopeCheck(_ scope: BsvHomeBsvMultisigActionScope?) throws {
    try Task.checkCancellation()
    try scope?.check()
}
