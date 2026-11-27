import Foundation
import enum BsvHomeCore.BsvHomeMpcError
import protocol BsvHomeCore.BsvHomeMpcParticipant
import class BsvHomeCore.BsvHomePlatformAccountService
import Keychain
import WowPayMpc

actor BsvHomeCoreMpcParticipantBridge: BsvHomeMpcParticipant {
    private let account: BsvHomePlatformAccountService
    private let environment: String
    private let keychain: any Keychain
    private var participant: WowPayMpcParticipant?
    private var scope: String?
    private var wallet: String?

    init(account: BsvHomePlatformAccountService, environment: String, keychain: any Keychain) {
        self.account = account
        self.environment = environment
        self.keychain = keychain
    }

    func exchange(walletId: String, messageJson: String) async throws -> String {
        guard let session = try await Task.detached(operation: { try self.account.session() }).value else {
            throw BsvHomeMpcError.Platform(error: .LoginRequired)
        }
        let namespace = environment + "\n" + session.userId
        if scope != namespace || wallet != walletId {
            try await closeCurrent()
            do {
                participant = try await WowPayMpcParticipant.open(walletId: walletId, namespace: namespace, keychain: keychain)
                scope = namespace
                wallet = walletId
            } catch { throw BsvHomeMpcError.SecureStorageUnavailable }
        }
        guard let participant else { throw BsvHomeMpcError.NativeUnavailable }
        do { return try await participant.exchange(messageJson) }
        catch { throw BsvHomeMpcError.NativeUnavailable }
    }

    func close(walletId: String) async throws {
        guard wallet == walletId else { return }
        try await closeCurrent()
    }

    private func closeCurrent() async throws {
        guard let participant else { return }
        do { try await participant.close() }
        catch { throw BsvHomeMpcError.SecureStorageUnavailable }
        if self.participant === participant {
            self.participant = nil
            scope = nil
            wallet = nil
        }
    }
}
