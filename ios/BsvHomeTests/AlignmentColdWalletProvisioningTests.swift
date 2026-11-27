import CryptoKit
import Foundation
@testable import BsvHome
import BsvHomeCore
import BsvHomeCorePrimitives
import BsvHomeCoreServices
import GRDB
import Keychain
import LocalAuthentication
import Primitives
import Store
import Testing
import UIKit

@MainActor
struct AlignmentColdWalletProvisioningTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA"] == "1", "Requires explicit provisioning of the three isolated cold QA simulators"))
    func provisionOneRealColdRoleInItsAuthorizedBaseAppContainer() async {
        var stage = Stage.host
        do {
            guard let role = Role(rawValue: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA_ROLE"] ?? "") else { throw Failure.rejected }
            let support = try validateHost(role)
            let publicDirectory = support.appendingPathComponent("AlignmentColdQa", isDirectory: true)
            let markerURL = publicDirectory.appendingPathComponent("public-marker.json")
            let exchangeURL = publicDirectory.appendingPathComponent("pairing-exchange.json")
            let marker = try readOptional(Marker.self, at: markerURL)
            let requiredPurpose: DevicePurpose = role == .legacyCold ? .coldWallet : .daily
            let existingPurpose = try DevicePurposeStore.appGroup.read()
            if let existingPurpose {
                guard existingPurpose == requiredPurpose else { throw Failure.rejected }
            }
            if marker != nil {
                guard existingPurpose == requiredPurpose else { throw Failure.rejected }
            }
            try validatePublicDirectory(publicDirectory, role: role, marker: marker)
            if let marker {
                guard marker.identifier == Self.identifier, marker.formatVersion == 1, marker.role == role,
                      marker.simulatorId == role.simulatorId, !marker.backupUiVerified, marker.testOnlyImportAuthentication
                else { throw Failure.rejected }
            }
            let incoming: Exchange?
            if role == .observer {
                incoming = try readRequired(Exchange.self, at: publicDirectory.appendingPathComponent("incoming-pairing.json"))
                guard let incoming, incoming.identifier == Self.identifier, incoming.formatVersion == 1,
                      incoming.sourceRole == .unifiedSigner, incoming.sourceSimulatorId == Role.unifiedSigner.simulatorId,
                      incoming.pairingHash == digest(incoming.pairingJson)
                else { throw Failure.rejected }
            } else {
                incoming = nil
            }
            stage = .storage
            guard let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else { throw Failure.rejected }
            for name in ["db.sqlite", "keystore"] {
                guard try !exists(documents.appendingPathComponent(name)) else { throw Failure.rejected }
            }
            let context = try makeContext(role: role, support: support, isResuming: marker != nil)
            let before = try context.walletStore.getWallets()
            if let marker {
                guard before.count == 1, before[0].id.id == marker.walletId, before[0].name == role.walletName,
                      try context.walletService.currentWalletId() == marker.walletId
                else { throw Failure.rejected }
                stage = .verification
                let verified = try await verify(role: role, wallet: before[0], context: context, incoming: incoming)
                let exchange = try readRequired(Exchange.self, at: exchangeURL)
                guard exchange == verified, marker.address == verified.address, marker.pairingHash == verified.pairingHash,
                      marker.secretStored == (role != .observer), context.provider.requestCount == 0
                else { throw Failure.rejected }
                let storedWalletsUnchanged = try context.walletStore.getWallets() == before
                #expect(storedWalletsUnchanged)
                #expect(context.provider.requestCount == 0)
                return
            }
            guard before.isEmpty, try !context.keystore.bsvHomeKeystore.hasStoredWallets(),
                  try isMissingOrEmptyDirectory(context.coldDirectory), !Task.isCancelled
            else { throw Failure.rejected }
            guard try DevicePurposeStore.appGroup.confirm(requiredPurpose) == requiredPurpose else { throw Failure.rejected }
            stage = .assetSeed
            try await initializeCanonicalNativeAssets(context: context)
            stage = .creationOrPublicImport
            let snapshot: BsvHomeBsvColdWalletSnapshot
            let generation = context.service.captureGeneration()
            if let incoming {
                let canonical = try canonicalPairing(incoming.pairingJson, keystore: context.keystore.bsvHomeKeystore)
                guard canonical == incoming.pairingJson, !Task.isCancelled else { throw Failure.rejected }
                snapshot = try await context.service.importObserver(name: role.walletName, pairingJson: canonical, expectedGeneration: generation)
            } else {
                snapshot = try await create(using: context.service, name: role.walletName, generation: generation)
            }
            guard !Task.isCancelled else { throw Failure.rejected }
            let after = try context.walletStore.getWallets()
            guard after.count == 1, after[0].id.id == snapshot.wallet.id,
                  try context.walletService.currentWalletId() == snapshot.wallet.id
            else { throw Failure.rejected }
            stage = .verification
            let exchange = try await verify(role: role, wallet: after[0], context: context, incoming: incoming)
            try context.keystore.applyFileProtection()
            if role == .legacyCold {
                try WalletFileProtection.apply(to: context.coldDirectory.deletingLastPathComponent())
            }
            guard context.provider.requestCount == 0, !Task.isCancelled else { throw Failure.rejected }
            let publicMarker = Marker(identifier: Self.identifier, formatVersion: 1, role: role, simulatorId: role.simulatorId, walletId: after[0].id.id, address: exchange.address, pairingHash: exchange.pairingHash, secretStored: role != .observer, backupUiVerified: false, testOnlyImportAuthentication: true)
            stage = .publicArtifacts
            try saveWithoutReplacing(exchange, at: exchangeURL)
            try saveWithoutReplacing(publicMarker, at: markerURL)
            #expect(context.provider.requestCount == 0)
            #expect(try readRequired(Marker.self, at: markerURL) == publicMarker)
        } catch {
            Issue.record("Cold QA provisioning was rejected or failed at \(stage.rawValue); wallet material is omitted")
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA_FINALIZE"] == "1", "Explicit readonly finalize of the interrupted existing Unified signer only"))
    func finalizeVerifiedSignerAfterInterruptedQaProvision() async {
        var verification = FinalizeVerification.host
        do {
            let role = Role.unifiedSigner
            guard ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA"] == "1",
                  ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA_ROLE"] == role.rawValue,
                  ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA_FINALIZE"] == "1"
            else { throw Failure.rejected }
            let support = try validateHost(role)
            guard try DevicePurposeStore.appGroup.read() == .daily else { throw Failure.rejected }
            let publicDirectory = support.appendingPathComponent("AlignmentColdQa", isDirectory: true)
            let markerURL = publicDirectory.appendingPathComponent("public-marker.json")
            let exchangeURL = publicDirectory.appendingPathComponent("pairing-exchange.json")
            verification = .publicFiles
            if try exists(publicDirectory) {
                let attributes = try FileManager.default.attributesOfItem(atPath: publicDirectory.path)
                let names = try Set(FileManager.default.contentsOfDirectory(atPath: publicDirectory.path))
                guard attributes[.type] as? FileAttributeType == .typeDirectory,
                      names.isSubset(of: ["pairing-exchange.json", "public-marker.json"]) else { throw Failure.rejected }
            }
            let oldMarker = try readOptional(Marker.self, at: markerURL)
            let oldExchange = try readOptional(Exchange.self, at: exchangeURL)
            guard oldMarker == nil || oldExchange != nil else { throw Failure.rejected }
            verification = .wallet
            let context = try makeContext(role: role, support: support, isResuming: true)
            let wallets = try context.walletStore.getWallets()
            guard wallets.count == 1, let wallet = wallets.first,
                  wallet.name == role.walletName, wallet.type == .single, wallet.source == .create,
                  wallet.accounts.count == 1, wallet.accounts[0].chain == .bitcoinSv else { throw Failure.rejected }
            verification = .selection
            guard try context.walletService.currentWalletId() == wallet.id.id else { throw Failure.rejected }
            verification = .coldState
            let beforeStamp = try context.keystore.bsvColdWalletStore.stateStamp(walletId: wallet.id.id)
            guard beforeStamp != nil,
                  try bsvColdWalletRole(wallet: wallet.map(), store: context.keystore.bsvColdWalletStore) == .signer else { throw Failure.rejected }
            let snapshot = try await context.service.snapshot(walletId: wallet.id.id)
            guard snapshot.wallet == wallet.map(), snapshot.role == .signer, snapshot.pending == nil else { throw Failure.rejected }
            verification = .pairing
            let pairing = try await context.service.pairing(walletId: wallet.id.id)
            guard try canonicalPairing(pairing, keystore: context.keystore.bsvHomeKeystore) == pairing else { throw Failure.rejected }
            verification = .encryptedSecretPresence
            guard try await context.walletService.secretKind(walletId: wallet.id.id) == .phrase,
                  context.keystore.bsvHomeKeystore.exists(keystoreId: context.keystore.bsvHomeKeystore.keystoreId(walletId: wallet.id.id)),
                  try context.keystore.bsvHomeKeystore.hasStoredWallets() else { throw Failure.rejected }
            verification = .verifier
            let secure = FinalizeReadOnlySecureStore(base: BsvHomeCoreSecurePreferencesStore(namespace: "offline_signing_password_", accessibility: .whenUnlockedThisDeviceOnly))
            let verifier = try BsvHomeOfflineSigningPasswordService(session: context.session, store: secure, wallet: wallet.map(), domain: .unifiedCold)
            let status = try await verifier.status()
            guard !status.configured, secure.mutationCount == 0 else { throw Failure.rejected }
            verification = .publicVerification
            let exchange = try await verify(role: role, wallet: wallet, context: context, incoming: nil)
            guard exchange.walletId == wallet.id.id, exchange.pairingJson == pairing else { throw Failure.rejected }
            let marker = Marker(identifier: Self.identifier, formatVersion: 1, role: role, simulatorId: role.simulatorId, walletId: wallet.id.id, address: exchange.address, pairingHash: exchange.pairingHash, secretStored: true, backupUiVerified: false, testOnlyImportAuthentication: true)
            guard oldExchange == nil || oldExchange == exchange, oldMarker == nil || oldMarker == marker else { throw Failure.rejected }
            verification = .unchanged
            let finalStatus = try await verifier.status()
            guard !finalStatus.configured, secure.mutationCount == 0,
                  try context.walletStore.getWallets() == wallets,
                  try context.walletService.currentWalletId() == wallet.id.id,
                  try context.keystore.bsvColdWalletStore.stateStamp(walletId: wallet.id.id) == beforeStamp,
                  context.provider.requestCount == 0, !Task.isCancelled else { throw Failure.rejected }
            verification = .publicOutput
            if oldExchange == nil {
                try saveWithoutReplacing(exchange, at: exchangeURL)
            }
            if oldMarker == nil {
                try saveWithoutReplacing(marker, at: markerURL)
            }
            #expect(try readRequired(Exchange.self, at: exchangeURL) == exchange)
            #expect(try readRequired(Marker.self, at: markerURL) == marker)
            #expect(secure.mutationCount == 0)
            #expect(context.provider.requestCount == 0)
        } catch {
            Issue.record("Interrupted Unified cold QA finalize stopped at \(verification.rawValue); existing wallet material was retained")
        }
    }

    private func canonicalPairing(_ descriptor: String, keystore: BsvHomeKeystore) throws -> String {
        let accountJSON = try keystore.bsvColdPairingValidate(descriptorJson: descriptor)
        let account = try JSONDecoder().decode(Primitives.Account.self, from: Data(accountJSON.utf8))
        return try keystore.bsvColdPairing(account: account.map())
    }

    private enum FinalizeVerification: String {
        case host
        case publicFiles
        case wallet
        case selection
        case coldState
        case pairing
        case encryptedSecretPresence
        case verifier
        case publicVerification
        case unchanged
        case publicOutput
    }

    private final class FinalizeReadOnlySecureStore: BsvHomeSecureStore, @unchecked Sendable {
        private let base: any BsvHomeSecureStore
        private let lock = NSLock()
        private var mutations = 0
        var mutationCount: Int { lock.withLock { mutations } }
        init(base: any BsvHomeSecureStore) {
            self.base = base
        }

        func get(key: String) throws -> String? {
            try base.get(key: key)
        }

        func set(key _: String, value _: String) throws {
            lock.withLock { mutations += 1 }
            throw BsvHomeServiceError.Unsupported(msg: "Interrupted QA finalize forbids verifier mutations")
        }

        func remove(key _: String) throws {
            lock.withLock { mutations += 1 }
            throw BsvHomeServiceError.Unsupported(msg: "Interrupted QA finalize forbids verifier mutations")
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL"] == "1" && ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL_STAGE"] == "prepare", "Explicit isolated observer synthetic Request stage only"))
    func prepareSyntheticRequestForTheExplicitColdLane() async {
        do {
            let lane = try protocolLane()
            let support = try validateHost(.observer)
            let directory = protocolDirectory(support, lane)
            let incoming = try readRequired(Exchange.self, at: directory.appendingPathComponent("incoming-pairing.json"))
            try validateProtocolPairing(incoming, lane: lane)
            let synthetic = try readRequired(SyntheticPrevout.self, at: directory.appendingPathComponent("synthetic-prevout.json"))
            try synthetic.validate(incoming: incoming, lane: lane)
            let configuration = ProtocolConfiguration(purpose: .prepare(synthetic))
            let context = try makeContext(role: .observer, support: support, isResuming: true, protocolConfiguration: configuration)
            let original = try await validateProvisionedProtocolHost(role: .observer, support: support, context: context)
            let walletMarkerURL = directory.appendingPathComponent("observer-wallet.json")
            let walletMarker: ProtocolWallet
            let walletId: String
            if let saved = try readOptional(ProtocolWallet.self, at: walletMarkerURL) {
                guard saved.identifier == Self.protocolIdentifier, saved.lane == lane, saved.pairingHash == incoming.pairingHash else { throw Failure.rejected }
                walletMarker = saved
                walletId = saved.walletId
            } else if lane == .unifiedSigner {
                guard original.pairingHash == incoming.pairingHash else { throw Failure.rejected }
                walletId = original.walletId
                walletMarker = ProtocolWallet(identifier: Self.protocolIdentifier, lane: lane, walletId: walletId, pairingHash: incoming.pairingHash)
                try saveWithoutReplacing(walletMarker, at: walletMarkerURL)
            } else {
                let snapshot = try await context.service.importObserver(name: "Alignment Legacy Observer Protocol", pairingJson: incoming.pairingJson, expectedGeneration: context.service.captureGeneration())
                guard snapshot.role == .observer, snapshot.pairingJson == incoming.pairingJson else { throw Failure.rejected }
                walletId = snapshot.wallet.id
                walletMarker = ProtocolWallet(identifier: Self.protocolIdentifier, lane: lane, walletId: walletId, pairingHash: incoming.pairingHash)
                try saveWithoutReplacing(walletMarker, at: walletMarkerURL)
            }
            try await validateProtocolObserver(walletId: walletId, pairing: incoming, context: context)
            try await context.service.selectWallet(walletId: walletId)
            let outputURL = directory.appendingPathComponent("request-exchange.json")
            if let saved = try readOptional(ProtocolRequest.self, at: outputURL) {
                try validateRequest(saved, lane: lane, pairing: incoming)
                let snapshot = try await context.service.snapshot(walletId: walletId)
                guard let pending = snapshot.pending, pending.requestJson == saved.requestJson, pending.signedJson == nil,
                      !pending.broadcasted, pending.review.commitment == saved.commitment else { throw Failure.rejected }
                try verifyFrames(saved.frames, payload: saved.requestJson, service: context.service)
                guard context.provider.requestCount == 0 else { throw Failure.rejected }
                return
            }
            guard try await context.service.snapshot(walletId: walletId).pending == nil else { throw Failure.rejected }
            let scope = try context.service.beginAction(walletId: walletId, intent: .prepare(recipient: Self.protocolRecipient, amount: Self.protocolAmount))
            let pending = try await context.service.prepare(walletId: walletId, recipient: Self.protocolRecipient, amount: Self.protocolAmount, scope: scope)
            guard pending.signedJson == nil, pending.transactionId == nil, !pending.broadcasted,
                  pending.review.recipient == Self.protocolRecipient, pending.review.amount == Self.protocolAmount,
                  pending.review.fee > 0, pending.review.totalDebit == pending.review.amount + pending.review.fee,
                  pending.review.inputCount == 1, pending.review.totalDebit < synthetic.valueSatoshis
            else { throw Failure.rejected }
            let frames = try context.service.qrFrames(payloadJson: pending.requestJson)
            try verifyFrames(frames, payload: pending.requestJson, service: context.service)
            let output = ProtocolRequest(identifier: Self.protocolIdentifier, lane: lane, pairingHash: incoming.pairingHash, sourceSignerWalletId: incoming.walletId, observerWalletId: walletId, requestHash: digest(pending.requestJson), requestJson: pending.requestJson, commitment: pending.review.commitment, frames: frames)
            try saveWithoutReplacing(output, at: outputURL)
            guard context.provider.requestCount > 0, context.provider.rejectedRequestCount == 0,
                  context.provider.broadcastAttemptCount == 0, try !context.keystore.bsvHomeKeystore.hasStoredWallets()
            else { throw Failure.rejected }
            #expect(configuration.statistics.authorizationCount == 0)
            #expect(configuration.statistics.systemInteractionCount == 0)
        } catch {
            Issue.record("Isolated cold protocol prepare stopped; public artifact or retained local state requires review")
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL"] == "1" && ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL_STAGE"] == "sign", "Explicit isolated real Native queue signing stage only; no hardware-auth claim"))
    func signThePublicRequestWithTheActualColdKeystoreAndNativeQueue() async {
        do {
            let lane = try protocolLane()
            let role = lane.role
            let support = try validateHost(role)
            let directory = protocolDirectory(support, lane)
            let configuration = ProtocolConfiguration(purpose: .sign)
            let context = try makeContext(role: role, support: support, isResuming: true, protocolConfiguration: configuration)
            let own = try await validateProvisionedProtocolHost(role: role, support: support, context: context)
            let incoming = try readRequired(ProtocolRequest.self, at: directory.appendingPathComponent("incoming-request.json"))
            try validateRequest(incoming, lane: lane, pairing: own)
            guard incoming.sourceSignerWalletId == own.walletId else { throw Failure.rejected }
            try verifyFrames(incoming.frames, payload: incoming.requestJson, service: context.service)
            try await context.service.selectWallet(walletId: own.walletId)
            let outputURL = directory.appendingPathComponent("signed-exchange.json")
            if let saved = try readOptional(ProtocolSigned.self, at: outputURL) {
                try validateSigned(saved, request: incoming, pairing: own, lane: lane)
                let snapshot = try await context.service.snapshot(walletId: own.walletId)
                guard snapshot.pending?.signedJson == saved.signedJson, snapshot.pending?.transactionId == saved.transactionId,
                      snapshot.pending?.requestJson == incoming.requestJson, snapshot.pending?.broadcasted == false,
                      configuration.statistics.authorizationCount == 0, configuration.statistics.systemInteractionCount == 0,
                      context.provider.requestCount == 0 else { throw Failure.rejected }
                try verifyFrames(saved.frames, payload: saved.signedJson, service: context.service)
                return
            }
            let snapshot = try await context.service.snapshot(walletId: own.walletId)
            let pending: BsvHomeBsvColdWalletPending
            if let retained = snapshot.pending {
                guard retained.requestJson == incoming.requestJson, retained.review.commitment == incoming.commitment else { throw Failure.rejected }
                pending = retained
            } else {
                pending = try await context.service.importRequest(walletId: own.walletId, requestJson: incoming.requestJson)
            }
            guard pending.review.recipient == Self.protocolRecipient, pending.review.amount == Self.protocolAmount,
                  pending.review.commitment == incoming.commitment, pending.review.inputCount == 1, !pending.broadcasted,
                  pending.review.totalDebit == pending.review.amount + pending.review.fee else { throw Failure.rejected }
            let result: BsvHomeBsvColdWalletPending
            if pending.signedJson != nil {
                result = pending
            } else {
                let scope = try context.service.beginAction(walletId: own.walletId, intent: .sign(commitment: pending.review.commitment))
                result = try await context.service.sign(walletId: own.walletId, commitment: pending.review.commitment, scope: scope)
                guard configuration.statistics.authorizationCount == 1, configuration.statistics.systemInteractionCount == 1 else { throw Failure.rejected }
            }
            guard let signed = result.signedJson, let transactionId = result.transactionId, !result.broadcasted,
                  result.review == pending.review, result.requestJson == incoming.requestJson,
                  context.provider.requestCount == 0 else { throw Failure.rejected }
            let frames = try context.service.qrFrames(payloadJson: signed)
            try verifyFrames(frames, payload: signed, service: context.service)
            let output = ProtocolSigned(identifier: Self.protocolIdentifier, lane: lane, pairingHash: own.pairingHash, sourceSignerWalletId: own.walletId, requestHash: incoming.requestHash, commitment: result.review.commitment, signedJson: signed, transactionId: transactionId, frames: frames)
            try saveWithoutReplacing(output, at: outputURL)
            #expect(try bsvColdWalletRole(wallet: snapshot.wallet, store: context.keystore.bsvColdWalletStore) == .signer)
            #expect(context.provider.requestCount == 0)
        } catch {
            Issue.record("Isolated cold protocol real-sign stage stopped; no verifier, Signed, or wallet state was reset")
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL"] == "1" && ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL_STAGE"] == "accept", "Explicit isolated observer verifies Signed only; no broadcast"))
    func importAndVerifySignedOnTheObserverWithoutBroadcasting() async {
        do {
            let lane = try protocolLane()
            let support = try validateHost(.observer)
            let directory = protocolDirectory(support, lane)
            let context = try makeContext(role: .observer, support: support, isResuming: true)
            _ = try await validateProvisionedProtocolHost(role: .observer, support: support, context: context)
            let own = try readRequired(Exchange.self, at: directory.appendingPathComponent("incoming-pairing.json"))
            try validateProtocolPairing(own, lane: lane)
            let request = try readRequired(ProtocolRequest.self, at: directory.appendingPathComponent("request-exchange.json"))
            let signed = try readRequired(ProtocolSigned.self, at: directory.appendingPathComponent("incoming-signed.json"))
            try validateRequest(request, lane: lane, pairing: own)
            try validateSigned(signed, request: request, pairing: own, lane: lane)
            try verifyFrames(signed.frames, payload: signed.signedJson, service: context.service)
            try await validateProtocolObserver(walletId: request.observerWalletId, pairing: own, context: context)
            let before = try await context.service.snapshot(walletId: request.observerWalletId)
            guard before.pending?.requestJson == request.requestJson, before.pending?.broadcasted == false else { throw Failure.rejected }
            let outputURL = directory.appendingPathComponent("accepted-public-marker.json")
            if let accepted = try readOptional(ProtocolAccepted.self, at: outputURL) {
                guard accepted.identifier == Self.protocolIdentifier, accepted.lane == lane, accepted.requestHash == request.requestHash,
                      accepted.transactionId == signed.transactionId, !accepted.broadcasted,
                      before.pending?.signedJson == signed.signedJson, before.pending?.transactionId == signed.transactionId,
                      context.provider.requestCount == 0 else { throw Failure.rejected }
                return
            }
            try await context.service.selectWallet(walletId: request.observerWalletId)
            let accepted = try await context.service.importSigned(walletId: request.observerWalletId, signedJson: signed.signedJson)
            guard accepted.signedJson == signed.signedJson, accepted.transactionId == signed.transactionId,
                  accepted.review.commitment == request.commitment, accepted.review.recipient == Self.protocolRecipient,
                  accepted.review.amount == Self.protocolAmount, accepted.requestJson == request.requestJson,
                  accepted.review == before.pending?.review, !accepted.broadcasted,
                  accepted.persistenceError == nil, context.provider.requestCount == 0,
                  try !context.keystore.bsvHomeKeystore.hasStoredWallets() else { throw Failure.rejected }
            let stamp = try context.keystore.bsvColdWalletStore.stateStamp(walletId: request.observerWalletId)
            let restored = try await context.service.snapshot(walletId: request.observerWalletId)
            guard restored.pending == accepted, try context.keystore.bsvColdWalletStore.stateStamp(walletId: request.observerWalletId) == stamp else { throw Failure.rejected }
            try saveWithoutReplacing(ProtocolAccepted(identifier: Self.protocolIdentifier, lane: lane, requestHash: request.requestHash, pairingHash: own.pairingHash, transactionId: signed.transactionId, amountSatoshis: accepted.review.amount, feeSatoshis: accepted.review.fee, totalDebitSatoshis: accepted.review.totalDebit, broadcasted: false, realNativeQueueAndSdk: true, hardwareAuthenticationVerified: false), at: outputURL)
            #expect(context.provider.requestCount == 0)
        } catch {
            Issue.record("Isolated cold protocol observer verification stopped; no broadcast or state deletion was attempted")
        }
    }

    private static let protocolIdentifier = "bsvhome-cold-protocol-qa-v1"
    private static let protocolRecipient = "1BitcoinEaterAddressDontSendf59kuE"
    private static let protocolAmount: UInt64 = 75000

    private func protocolLane() throws -> ProtocolLane {
        guard ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA"] == "1",
              ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL"] == "1",
              let lane = ProtocolLane(rawValue: ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL_LANE"] ?? "")
        else { throw Failure.rejected }
        return lane
    }

    private func protocolDirectory(_ support: URL, _ lane: ProtocolLane) -> URL {
        support.appendingPathComponent("AlignmentColdQaProtocol", isDirectory: true).appendingPathComponent(lane.rawValue, isDirectory: true)
    }

    private func validateProvisionedProtocolHost(role: Role, support: URL, context: Context) async throws -> Exchange {
        let directory = support.appendingPathComponent("AlignmentColdQa", isDirectory: true)
        let marker = try readRequired(Marker.self, at: directory.appendingPathComponent("public-marker.json"))
        let exchange = try readRequired(Exchange.self, at: directory.appendingPathComponent("pairing-exchange.json"))
        guard marker.identifier == Self.identifier, marker.formatVersion == 1, marker.role == role, marker.simulatorId == role.simulatorId,
              !marker.backupUiVerified, marker.testOnlyImportAuthentication,
              marker.secretStored == (role != .observer), marker.walletId == exchange.walletId,
              marker.address == exchange.address, marker.pairingHash == exchange.pairingHash,
              try DevicePurposeStore.appGroup.read() == (role == .legacyCold ? .coldWallet : .daily)
        else { throw Failure.rejected }
        let wallets = try context.walletStore.getWallets()
        guard let original = wallets.first(where: { $0.id.id == marker.walletId }),
              try await verify(role: role, wallet: original, context: context, incoming: nil) == exchange else { throw Failure.rejected }
        var allowed = Set([original.id.id])
        if role == .observer {
            for lane in ProtocolLane.allCases {
                if let record = try readOptional(ProtocolWallet.self, at: protocolDirectory(support, lane).appendingPathComponent("observer-wallet.json")) {
                    guard record.identifier == Self.protocolIdentifier, record.lane == lane,
                          let wallet = wallets.first(where: { $0.id.id == record.walletId }), wallet.type == .view,
                          try await context.walletService.secretKind(walletId: record.walletId) == nil,
                          try await digest(context.service.pairing(walletId: record.walletId)) == record.pairingHash
                    else { throw Failure.rejected }
                    allowed.insert(record.walletId)
                }
            }
        }
        guard Set(wallets.map(\.id.id)) == allowed, wallets.count == allowed.count, context.provider.requestCount == 0 else { throw Failure.rejected }
        return exchange
    }

    private func validateProtocolObserver(walletId: String, pairing: Exchange, context: Context) async throws {
        let snapshot = try await context.service.snapshot(walletId: walletId)
        guard snapshot.role == .observer, snapshot.wallet.walletType == .view,
              snapshot.pairingJson == pairing.pairingJson,
              snapshot.wallet.accounts.count == 1, snapshot.wallet.accounts[0].address == pairing.address,
              try await context.walletService.secretKind(walletId: walletId) == nil,
              !context.keystore.bsvHomeKeystore.exists(keystoreId: context.keystore.bsvHomeKeystore.keystoreId(walletId: walletId)),
              try !context.keystore.bsvHomeKeystore.hasStoredWallets() else { throw Failure.rejected }
    }

    private func validateProtocolPairing(_ pairing: Exchange, lane: ProtocolLane) throws {
        guard pairing.identifier == Self.identifier, pairing.formatVersion == 1,
              pairing.sourceRole == lane.role, pairing.sourceSimulatorId == lane.role.simulatorId,
              pairing.pairingHash == digest(pairing.pairingJson), !pairing.walletId.isEmpty,
              try JSONDecoder().decode(PublicPairing.self, from: Data(pairing.pairingJson.utf8)).firstAddress == pairing.address
        else { throw Failure.rejected }
    }

    private func validateRequest(_ request: ProtocolRequest, lane: ProtocolLane, pairing: Exchange) throws {
        try validateProtocolPairing(pairing, lane: lane)
        guard request.identifier == Self.protocolIdentifier, request.lane == lane, request.pairingHash == pairing.pairingHash,
              request.sourceSignerWalletId == pairing.walletId, request.requestHash == digest(request.requestJson),
              !request.observerWalletId.isEmpty, !request.commitment.isEmpty else { throw Failure.rejected }
    }

    private func validateSigned(_ signed: ProtocolSigned, request: ProtocolRequest, pairing: Exchange, lane: ProtocolLane) throws {
        try validateRequest(request, lane: lane, pairing: pairing)
        guard signed.identifier == Self.protocolIdentifier, signed.lane == lane,
              signed.pairingHash == pairing.pairingHash, signed.sourceSignerWalletId == pairing.walletId,
              signed.requestHash == request.requestHash, signed.commitment == request.commitment,
              signed.transactionId.count == 64, signed.transactionId.allSatisfy({ $0.isHexDigit && !$0.isUppercase }),
              !signed.signedJson.isEmpty else { throw Failure.rejected }
    }

    private func verifyFrames(_ frames: [String], payload: String, service: BsvHomeBsvColdWalletService) throws {
        guard !frames.isEmpty, frames.count <= 100, try service.qrFrames(payloadJson: payload) == frames,
              try service.qrAssemble(frames: Array(frames.reversed())) == payload else { throw Failure.rejected }
    }

    private enum ProtocolLane: String, Codable, CaseIterable, Sendable {
        case unifiedSigner
        case legacyCold
        var role: Role { self == .unifiedSigner ? .unifiedSigner : .legacyCold }
    }

    private struct ProtocolWallet: Codable {
        let identifier: String
        let lane: ProtocolLane
        let walletId: String
        let pairingHash: String
    }

    private struct ProtocolRequest: Codable {
        let identifier: String
        let lane: ProtocolLane
        let pairingHash: String
        let sourceSignerWalletId: String
        let observerWalletId: String
        let requestHash: String
        let requestJson: String
        let commitment: String
        let frames: [String]
    }

    private struct ProtocolSigned: Codable {
        let identifier: String
        let lane: ProtocolLane
        let pairingHash: String
        let sourceSignerWalletId: String
        let requestHash: String
        let commitment: String
        let signedJson: String
        let transactionId: String
        let frames: [String]
    }

    private struct ProtocolAccepted: Codable {
        let identifier: String
        let lane: ProtocolLane
        let requestHash: String
        let pairingHash: String
        let transactionId: String
        let amountSatoshis: UInt64
        let feeSatoshis: UInt64
        let totalDebitSatoshis: UInt64
        let broadcasted: Bool
        let realNativeQueueAndSdk: Bool
        let hardwareAuthenticationVerified: Bool
    }

    private final class ProtocolStatistics: @unchecked Sendable {
        private let lock = NSLock()
        private var authorizations = 0
        private var systemInteractions = 0
        var authorizationCount: Int { lock.withLock { authorizations } }
        var systemInteractionCount: Int { lock.withLock { systemInteractions } }
        func authorization() {
            lock.withLock { authorizations += 1 }
        }

        func systemInteraction() {
            lock.withLock { systemInteractions += 1 }
        }
    }

    private enum ProtocolPurpose: Sendable {
        case prepare(SyntheticPrevout)
        case sign
    }

    private final class ProtocolConfiguration: @unchecked Sendable {
        let purpose: ProtocolPurpose
        let statistics = ProtocolStatistics()
        init(purpose: ProtocolPurpose) {
            self.purpose = purpose
        }

        func authorize(scope: BsvHomeBsvColdWalletActionScope, store: any BsvHomeSecureStore, role: Role) async throws {
            guard case .sign = purpose else { throw Failure.rejected }
            try await ProtocolSession.check(role: role)
            try await scope.validate()
            let verifier = try BsvHomeOfflineSigningPasswordService.forScope(store: store, scope: scope)
            try await ProtocolSession.check(role: role)
            let status = try await verifier.status()
            try await ProtocolSession.check(role: role)
            guard !status.configured else { throw Failure.rejected }
            var password = try SecureRandom.generateKey(length: 32).hex
            defer { password = "" }
            try await ProtocolSession.check(role: role)
            _ = try await verifier.set(snapshot: status, currentPassword: nil, newPassword: password, confirmation: password)
            try await ProtocolSession.check(role: role)
            try await scope.validate()
            statistics.authorization()
        }
    }

    private enum ProtocolSession {
        @MainActor static func check(role: Role) throws {
            guard ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA"] == "1",
                  ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL"] == "1",
                  ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_PROTOCOL_STAGE"] == "sign",
                  ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == role.simulatorId,
                  BsvHomeAppHostMode.isUnitTestHost, Bundle.main.bundleIdentifier == "com.bsvhome.ios", !Task.isCancelled,
                  UIApplication.shared.applicationState == .active, UIApplication.shared.isProtectedDataAvailable,
                  !UIScreen.main.isCaptured else { throw Failure.rejected }
        }
    }

    private struct ProtocolSystemInteraction: BiometryAuthenticatable {
        let role: Role
        let statistics: ProtocolStatistics
        var requiresAuthentication: Bool { true }
        var availableAuthentication: KeystoreAuthentication { .none }
        var lockPeriod: LockPeriod { .default }
        var isPrivacyLockEnabled: Bool { false }
        @MainActor func authenticate(context _: LAContext, reason _: String) async throws {
            try ProtocolSession.check(role: role)
            statistics.systemInteraction()
        }

        @MainActor func enableAuthentication(_: Bool, context _: LAContext, reason _: String) async throws {
            throw Failure.rejected
        }

        func update(period _: LockPeriod) throws {
            throw Failure.rejected
        }

        func togglePrivacyLock(enabled _: Bool) throws {
            throw Failure.rejected
        }

        func shouldRelock(elapsedMilliseconds _: Int64) -> Bool {
            true
        }
    }

    private struct SyntheticPrevout: Codable, Sendable {
        let identifier: String
        let formatVersion: Int
        let lane: ProtocolLane
        let pairingHash: String
        let address: String
        let previousRaw: String
        let previousId: String
        let scriptHex: String
        let valueSatoshis: UInt64

        func validate(incoming: Exchange, lane: ProtocolLane) throws {
            guard identifier == "bsvhome-cold-synthetic-prevout-v1", formatVersion == 1, self.lane == lane,
                  pairingHash == incoming.pairingHash, address == incoming.address, valueSatoshis == 100_000,
                  scriptHex.count == 50, scriptHex.hasPrefix("76a914"), scriptHex.hasSuffix("88ac"),
                  previousRaw.count < 4096, previousRaw.count.isMultiple(of: 2), previousId.count == 64
            else { throw Failure.rejected }
            var bytes = Data()
            var cursor = previousRaw.startIndex
            while cursor < previousRaw.endIndex {
                let end = previousRaw.index(cursor, offsetBy: 2)
                guard let byte = UInt8(previousRaw[cursor ..< end], radix: 16) else { throw Failure.rejected }
                bytes.append(byte)
                cursor = end
            }
            let computed = SHA256.hash(data: Data(SHA256.hash(data: bytes))).reversed().map { String(format: "%02x", $0) }.joined()
            guard computed == previousId else { throw Failure.rejected }
        }
    }

    private static let identifier = "bsvhome-cold-alignment-qa-v1"
    private static let runtimeEnvironment: BsvHomeRuntimeEnvironment = .wowPay
    private static let maximumPublicBytes = 65536

    private func validateHost(_ role: Role) throws -> URL {
        #if DEBUG && targetEnvironment(simulator)
            guard BsvHomeAppHostMode.isUnitTestHost,
                  ProcessInfo.processInfo.environment["SIMULATOR_UDID"]?.uppercased() == role.simulatorId,
                  Bundle.main.bundleIdentifier == "com.bsvhome.ios", Bundle.main.bundleURL.lastPathComponent == "BsvHome.app",
                  Bundle.main.executableURL?.lastPathComponent == "BsvHome"
            else { throw Failure.rejected }
            let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true).resolvingSymlinksInPath()
            let tail = Array(home.pathComponents.suffix(7))
            guard tail.count == 7, Array(tail.prefix(6)) == ["Devices", role.simulatorId, "data", "Containers", "Data", "Application"],
                  UUID(uuidString: tail[6]) != nil,
                  let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first,
                  support.resolvingSymlinksInPath().path.hasPrefix(home.path + "/")
            else { throw Failure.rejected }
            return support
        #else
            throw Failure.rejected
        #endif
    }

    private func validatePublicDirectory(_ directory: URL, role: Role, marker: Marker?) throws {
        guard try exists(directory) else { return }
        let attributes = try FileManager.default.attributesOfItem(atPath: directory.path)
        guard attributes[.type] as? FileAttributeType == .typeDirectory else { throw Failure.rejected }
        let names = try Set(FileManager.default.contentsOfDirectory(atPath: directory.path))
        let allowed: Set<String> = marker == nil ? (role == .observer ? ["incoming-pairing.json"] : []) : (role == .observer ? ["incoming-pairing.json", "pairing-exchange.json", "public-marker.json"] : ["pairing-exchange.json", "public-marker.json"])
        guard names == allowed else { throw Failure.rejected }
    }

    private func makeContext(role: Role, support: URL, isResuming: Bool, protocolConfiguration: ProtocolConfiguration? = nil) throws -> Context {
        let database: DB
        let keystore: LocalKeystore
        let preferencesStore: BsvHomeCorePreferencesStore
        let walletPreferences: BsvHomeCoreWalletPreferencesStore
        let coldDirectory: URL
        if role == .legacyCold {
            for name in ["db.sqlite", "keystore", "WowPayColdWallet"] {
                guard try !exists(support.appendingPathComponent(name)) else { throw Failure.rejected }
            }
            let directory = support.appendingPathComponent("ColdWallet", isDirectory: true)
            if !isResuming {
                guard try !exists(directory) else { throw Failure.rejected }
            }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.protectionKey: FileProtectionType.complete])
            try WalletFileProtection.apply(to: directory)
            let configuration: GRDB.Configuration = DB.defaultConfiguration
            database = try DB(fileURL: directory.appendingPathComponent("offline.sqlite"), configuration: configuration)
            let keychain = KeychainDefault(service: "com.bsvhome.ios.cold-wallet")
            let password = LocalKeystorePassword(keychain: keychain)
            coldDirectory = directory.appendingPathComponent("Transactions", isDirectory: true)
            let store = try BsvHomeCoreBsvColdWalletStore(directory: coldDirectory)
            keystore = try LocalKeystore(directoryURL: directory.appendingPathComponent("Keystore", isDirectory: true), keystorePassword: password, environment: Self.runtimeEnvironment, coldWalletStore: store)
            preferencesStore = BsvHomeCorePreferencesStore(namespace: "cold_wallet_")
            walletPreferences = BsvHomeCoreWalletPreferencesStore(namespace: "cold_wallet_")
        } else {
            guard try !exists(support.appendingPathComponent("ColdWallet", isDirectory: true)) else { throw Failure.rejected }
            let resolver = AppResolver.main
            database = resolver.storages.db
            keystore = resolver.storages.keystore
            preferencesStore = BsvHomeCorePreferencesStore.application()
            walletPreferences = BsvHomeCoreWalletPreferencesStore()
            coldDirectory = support.appendingPathComponent("WowPayColdWallet", isDirectory: true)
        }
        let walletStore = WalletStore(db: database)
        let nativeWalletStore = BsvHomeCoreWalletStore(store: walletStore)
        let session = BsvHomeWalletSessionService.newWithEnvironment(store: BsvHomeCoreWalletSessionStore(store: preferencesStore), wallets: nativeWalletStore, environment: Self.runtimeEnvironment)
        let provider = switch protocolConfiguration?.purpose {
        case let .some(.prepare(synthetic)):
            RejectingProvider(synthetic: synthetic)
        case .some(.sign), .none:
            RejectingProvider()
        }
        let preferences = BsvHomePreferencesService(store: preferencesStore)
        let gateway = BsvHomeGateway.newWithEnvironment(provider: provider, preferences: preferencesStore, securePreferences: RejectingSecureStore(), environment: Self.runtimeEnvironment)
        let password = BsvHomeCoreKeystorePassword(keystore: keystore)
        let addresses = BsvHomeBsvAddressPool(gateway: gateway, walletStore: nativeWalletStore, store: BsvHomeCoreBsvAddressStore(store: BsvAddressPoolStore(db: database)), keystore: keystore.bsvHomeKeystore, password: password)
        let wallets = BsvHomeWalletService(keystore: keystore.bsvHomeKeystore, password: password, store: nativeWalletStore, session: session, appPreferences: preferences, files: BsvHomeCoreFileStore(), preferences: BsvHomeWalletPreferencesService(store: walletPreferences), explorer: BsvHomeExplorerService(preferences: preferences), addresses: BsvHomeCoreAddressStore(store: AddressStore(db: database)), localizer: BsvHomeCoreLocalizer(), bsvAddresses: addresses)
        let signer: any BsvHomeBsvColdWalletSigner
        if let configuration = protocolConfiguration, case .sign = configuration.purpose {
            let keychain = role == .legacyCold ? KeychainDefault(service: "com.bsvhome.ios.cold-wallet") : KeychainDefault()
            let secure = BsvHomeCoreSecurePreferencesStore(namespace: "offline_signing_password_", keychain: keychain, accessibility: .whenUnlockedThisDeviceOnly)
            signer = BsvHomeCoreBsvColdWalletSigner(keystore: keystore, authenticator: ProtocolSystemInteraction(role: role, statistics: configuration.statistics), reason: "Isolated cold protocol QA", authorizeSigning: { _, scope in
                try await configuration.authorize(scope: scope, store: secure, role: role)
            })
        } else {
            signer = ProvisioningOnlySigner()
        }
        let service = role == .legacyCold ? BsvHomeBsvColdWalletService.offline(wallet: wallets, addresses: addresses, keystore: keystore.bsvHomeKeystore, store: keystore.bsvColdWalletStore, signer: signer) : BsvHomeBsvColdWalletService.unified(wallet: wallets, addresses: addresses, gateway: gateway, keystore: keystore.bsvHomeKeystore, store: keystore.bsvColdWalletStore, signer: signer)
        let assetStore = Store.AssetStore(db: database)
        let nativeAssets = BsvHomeCoreAssetStore(assetStore: assetStore, balanceStore: BalanceStore(db: database))
        return Context(service: service, session: session, walletService: wallets, walletStore: walletStore, assetStore: assetStore, nativeAssets: nativeAssets, keystore: keystore, coldDirectory: coldDirectory, provider: provider)
    }

    private func canonicalNativeAssets() throws -> [BsvHomeCore.AssetBasic] {
        let config = BsvHomeAssetConfigService()
        let chains = BsvHomeChainService.newWithEnvironment(environment: Self.runtimeEnvironment).getChains(query: "")
        guard chains.contains(Primitives.Chain.bitcoinSv.rawValue) else { throw Failure.rejected }
        return chains.map { config.defaultAssetBasic(asset: config.chainAsset(chain: $0).asset) }
    }

    private func initializeCanonicalNativeAssets(context: Context) async throws {
        for canonical in try canonicalNativeAssets() {
            let existing = try context.assetStore.getAssets(for: [canonical.asset.id])
            if existing.isEmpty {
                try await context.nativeAssets.saveAssets(assets: [canonical])
            }
            guard try context.assetStore.getAssets(for: [canonical.asset.id]) == [canonical.asset.map()] else { throw Failure.rejected }
        }
    }

    private func create(using service: BsvHomeBsvColdWalletService, name: String, generation: UInt64) async throws -> BsvHomeBsvColdWalletSnapshot {
        var words = try service.createWords(expectedGeneration: generation)
        defer { words.removeAll(keepingCapacity: false) }
        guard words.count == 12, !Task.isCancelled else { throw Failure.rejected }
        return try await service.create(name: name, words: words, expectedGeneration: generation)
    }

    private func verify(role: Role, wallet: Primitives.Wallet, context: Context, incoming: Exchange?) async throws -> Exchange {
        for basic in try canonicalNativeAssets() {
            guard try context.assetStore.getAssets(for: [basic.asset.id]) == [basic.asset.map()] else { throw Failure.rejected }
        }
        guard wallet.name == role.walletName, wallet.accounts.count == 1,
              let account = wallet.accounts.first, account.chain == .bitcoinSv,
              wallet.type == (role == .observer ? .view : .single), wallet.source == (role == .observer ? .import : .create)
        else { throw Failure.rejected }
        let snapshot = try await context.service.snapshot(walletId: wallet.id.id)
        let expectedRole: BsvHomeBsvColdWalletRole = role == .observer ? .observer : .signer
        guard snapshot.role == expectedRole,
              try bsvColdWalletRole(wallet: wallet.map(), store: context.keystore.bsvColdWalletStore) == expectedRole,
              try await context.keystore.bsvColdWalletStore.get(walletId: wallet.id.id) != nil
        else { throw Failure.rejected }
        let pairing = try await context.service.pairing(walletId: wallet.id.id)
        let canonical = try canonicalPairing(pairing, keystore: context.keystore.bsvHomeKeystore)
        guard canonical == pairing, !Task.isCancelled else { throw Failure.rejected }
        let descriptor = try JSONDecoder().decode(PublicPairing.self, from: Data(canonical.utf8))
        guard descriptor.firstAddress == account.address else { throw Failure.rejected }
        if let incoming {
            let source = try canonicalPairing(incoming.pairingJson, keystore: context.keystore.bsvHomeKeystore)
            guard source == canonical, incoming.address == account.address, incoming.pairingHash == digest(canonical) else { throw Failure.rejected }
        }
        let secret = try await context.walletService.secretKind(walletId: wallet.id.id)
        let stored = context.keystore.bsvHomeKeystore.exists(keystoreId: context.keystore.bsvHomeKeystore.keystoreId(walletId: wallet.id.id))
        if role == .observer {
            guard secret == nil, !stored, try !context.keystore.bsvHomeKeystore.hasStoredWallets() else { throw Failure.rejected }
        } else {
            guard secret == .phrase, stored, try context.keystore.bsvHomeKeystore.hasStoredWallets() else { throw Failure.rejected }
        }
        return Exchange(identifier: Self.identifier, formatVersion: 1, sourceRole: role, sourceSimulatorId: role.simulatorId, walletId: wallet.id.id, address: account.address, pairingJson: canonical, pairingHash: digest(canonical))
    }

    private func exists(_ url: URL) throws -> Bool {
        do { _ = try FileManager.default.attributesOfItem(atPath: url.path); return true }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile || error.code == .fileNoSuchFile { return false }
    }

    private func isMissingOrEmptyDirectory(_ url: URL) throws -> Bool {
        guard try exists(url) else { return true }
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard attributes[.type] as? FileAttributeType == .typeDirectory else { throw Failure.rejected }
        return try FileManager.default.contentsOfDirectory(atPath: url.path).isEmpty
    }

    private func readOptional<T: Decodable>(_ type: T.Type, at url: URL) throws -> T? {
        guard try exists(url) else { return nil }
        return try readRequired(type, at: url)
    }

    private func readRequired<T: Decodable>(_ type: T.Type, at url: URL) throws -> T {
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        guard attributes[.type] as? FileAttributeType == .typeRegular else { throw Failure.rejected }
        let data = try Data(contentsOf: url)
        guard data.count <= Self.maximumPublicBytes else { throw Failure.rejected }
        return try JSONDecoder().decode(type, from: data)
    }

    private func saveWithoutReplacing(_ value: some Encodable, at url: URL) throws {
        guard try !exists(url) else { throw Failure.rejected }
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try WalletFileProtection.apply(to: directory)
        let temporary = directory.appendingPathComponent("public-" + UUID().uuidString + ".tmp")
        defer { try? FileManager.default.removeItem(at: temporary) }
        let data = try JSONEncoder().encode(value)
        guard data.count <= Self.maximumPublicBytes else { throw Failure.rejected }
        try data.write(to: temporary, options: [.atomic, .completeFileProtection])
        try FileManager.default.moveItem(at: temporary, to: url)
        try WalletFileProtection.apply(to: url)
    }

    private func digest(_ value: String) -> String {
        SHA256.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    private enum Role: String, Codable, Sendable {
        case unifiedSigner
        case observer
        case legacyCold

        var simulatorId: String {
            switch self {
            case .unifiedSigner: "F47186EF-B7A9-4956-A2A2-C04663361205"
            case .observer: "4A910C7A-C540-40C9-9405-04F46F5E81C1"
            case .legacyCold: "1DE87C77-6CB7-4FB7-BB69-30242C8FEEF0"
            }
        }

        var walletName: String {
            switch self {
            case .unifiedSigner: "Alignment Cold Signer QA"
            case .observer: "Alignment Cold Observer QA"
            case .legacyCold: "Alignment Legacy Cold QA"
            }
        }
    }

    private struct Context {
        let service: BsvHomeBsvColdWalletService
        let session: BsvHomeWalletSessionService
        let walletService: BsvHomeWalletService
        let walletStore: WalletStore
        let assetStore: Store.AssetStore
        let nativeAssets: BsvHomeCoreAssetStore
        let keystore: LocalKeystore
        let coldDirectory: URL
        let provider: RejectingProvider
    }

    private struct PublicPairing: Decodable {
        let firstAddress: String
    }

    private struct Exchange: Codable, Equatable {
        let identifier: String
        let formatVersion: Int
        let sourceRole: Role
        let sourceSimulatorId: String
        let walletId: String
        let address: String
        let pairingJson: String
        let pairingHash: String
    }

    private struct Marker: Codable, Equatable {
        let identifier: String
        let formatVersion: Int
        let role: Role
        let simulatorId: String
        let walletId: String
        let address: String
        let pairingHash: String
        let secretStored: Bool
        let backupUiVerified: Bool
        let testOnlyImportAuthentication: Bool
    }

    private enum Stage: String {
        case host
        case storage
        case assetSeed
        case creationOrPublicImport
        case verification
        case publicArtifacts
    }

    private enum Failure: Error {
        case rejected
    }

    private final class RejectingProvider: AlienProvider, @unchecked Sendable {
        private let lock = NSLock()
        private let synthetic: SyntheticPrevout?
        private var requests = 0
        private var rejections = 0
        private var broadcasts = 0
        var requestCount: Int { lock.withLock { requests } }
        var rejectedRequestCount: Int { lock.withLock { rejections } }
        var broadcastAttemptCount: Int { lock.withLock { broadcasts } }
        init(synthetic: SyntheticPrevout? = nil) {
            self.synthetic = synthetic
        }

        func request(target: AlienTarget) async throws -> AlienResponse {
            lock.withLock { requests += 1 }
            do {
                guard let synthetic, let url = URLComponents(string: target.url), url.scheme == "https",
                      url.host == "api.whatsonchain.com", url.port == nil, url.user == nil, url.password == nil,
                      url.query == nil, url.fragment == nil,
                      (target.headers ?? [:]).keys.allSatisfy({ !["authorization", "cookie", "proxy-authorization"].contains($0.lowercased()) }),
                      url.path.hasPrefix("/v1/bsv/main/") else { throw Failure.rejected }
                let path = String(url.path.dropFirst("/v1/bsv/main".count))
                if path == "/tx/raw" {
                    lock.withLock { broadcasts += 1 }
                    throw Failure.rejected
                }
                let json: Any
                if target.method == .get, path == "/chain/info" {
                    json = ["chain": "main", "blocks": 1000, "headers": 1000]
                } else if target.method == .get, path == "/feerecommendation" {
                    json = ["fee_unit": "sat/KB", "fee": 1000, "mempool_min_fee": 1000]
                } else if target.method == .post, path == "/addresses/history/all", let body = target.body,
                          body.count <= 16384, let value = try JSONSerialization.jsonObject(with: body) as? [String: Any],
                          Set(value.keys) == ["addresses"], let addresses = value["addresses"] as? [String],
                          !addresses.isEmpty, addresses.count <= 100
                {
                    json = addresses.map { address in
                        let history: [[String: Any]] = address == synthetic.address ? [["tx_hash": synthetic.previousId, "height": 100]] : []
                        return ["address": address, "confirmed": ["result": history, "error": ""], "unconfirmed": ["result": [], "error": ""]] as [String: Any]
                    }
                } else if target.method == .get, path.hasPrefix("/address/"), path.hasSuffix("/unspent/all") {
                    let address = String(path.dropFirst("/address/".count).dropLast("/unspent/all".count))
                    guard !address.contains("/"), (25 ... 35).contains(address.count) else { throw Failure.rejected }
                    let unspent: [[String: Any]] = address == synthetic.address ? [["tx_hash": synthetic.previousId, "tx_pos": 0, "value": synthetic.valueSatoshis, "height": 100, "status": "confirmed", "isSpentInMempoolTx": false]] : []
                    json = ["address": address, "result": unspent, "error": ""]
                } else if target.method == .get, path == "/tx/hash/" + synthetic.previousId {
                    json = ["txid": synthetic.previousId, "confirmations": 901, "blockheight": 100, "blocktime": 1_700_000_000, "time": 1_700_000_000,
                            "vin": [["coinbase": "0107"]],
                            "vout": [["n": 0, "value": 0.001, "scriptPubKey": ["hex": synthetic.scriptHex]]]]
                } else if target.method == .get, path == "/tx/" + synthetic.previousId + "/hex" {
                    return AlienResponse(status: 200, data: Data(synthetic.previousRaw.utf8))
                } else {
                    throw Failure.rejected
                }
                return try AlienResponse(status: 200, data: JSONSerialization.data(withJSONObject: json, options: [.sortedKeys]))
            } catch {
                lock.withLock { rejections += 1 }
                throw AlienError.RequestError(msg: "Isolated protocol provider rejected a non-allowlisted local request")
            }
        }
    }

    private final class RejectingSecureStore: BsvHomeSecureStore {
        func get(key _: String) throws -> String? {
            throw BsvHomeServiceError.Unsupported(msg: "Cold QA provisioning forbids gateway credentials")
        }

        func set(key _: String, value _: String) throws {
            throw BsvHomeServiceError.Unsupported(msg: "Cold QA provisioning forbids gateway credentials")
        }

        func remove(key _: String) throws {
            throw BsvHomeServiceError.Unsupported(msg: "Cold QA provisioning forbids gateway credentials")
        }
    }

    private final class ProvisioningOnlySigner: BsvHomeBsvColdWalletSigner {
        func authenticateImport() async throws {
            guard ProcessInfo.processInfo.environment["BSVHOME_ALIGNMENT_COLD_QA"] == "1", !Task.isCancelled else { throw BsvHomeServiceError.Cancelled }
        }

        func authorize(wallet _: BsvHomeCore.Wallet, scope _: BsvHomeBsvColdWalletActionScope) async throws {
            throw denied()
        }

        func authenticate(wallet _: BsvHomeCore.Wallet, scope _: BsvHomeBsvColdWalletActionScope) async throws {
            throw denied()
        }

        func signRequest(wallet _: BsvHomeCore.Wallet, requestJson _: String, reviewJson _: String, scope _: BsvHomeBsvColdWalletActionScope) async throws -> String {
            throw denied()
        }

        private func denied() -> BsvHomeServiceError {
            .Unsupported(msg: "Cold QA provisioning cannot authorize, sign, or broadcast a transaction")
        }
    }
}
