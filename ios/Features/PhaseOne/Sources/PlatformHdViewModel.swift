import Foundation
import enum BsvHomeCore.BsvHomePlatformAccountError
import struct BsvHomeCore.BsvHomePlatformHdAccount
import struct BsvHomeCore.BsvHomePlatformHdLocalWallet
import struct BsvHomeCore.BsvHomePlatformHdScan
import protocol BsvHomeCore.BsvHomePlatformHdServiceProtocol
import struct BsvHomeCore.BsvHomePlatformHdSnapshot
import Localization
import Observation

@Observable
@MainActor
public final class PlatformHdViewModel {
    private let service: any BsvHomePlatformHdServiceProtocol
    private let onSessionInvalidated: @MainActor @Sendable () async -> Void
    private var revision: UInt = 0
    private(set) var isBusy = false
    private(set) var error: String?
    private(set) var wallets: [BsvHomePlatformHdLocalWallet] = []
    private(set) var accounts: [BsvHomePlatformHdAccount] = []
    private(set) var snapshot: BsvHomePlatformHdSnapshot?
    private(set) var scan: BsvHomePlatformHdScan?
    var localWalletId: String?
    var label = ""
    var consent = false
    var gapLimit = "20"
    var maximumIndex = "1000"

    public init(service: any BsvHomePlatformHdServiceProtocol, onSessionInvalidated: @escaping @MainActor @Sendable () async -> Void = {}) {
        self.service = service
        self.onSessionInvalidated = onSessionInvalidated
    }

    func load() async {
        guard let overview = await value({ try await $0.overview() }) else { return }
        wallets = overview.localWallets
        accounts = overview.accounts
        if !wallets.contains(where: { $0.id == localWalletId }) {
            selectLocal(wallets.first?.id)
        }
    }

    func selectLocal(_ id: String?) {
        localWalletId = id
        label = wallets.first(where: { $0.id == id })?.name ?? ""
        consent = false
        snapshot = nil
        scan = nil
    }

    func enroll() async {
        guard let localWalletId else { error = Localized.phaseoneHdSelectWallet; return }
        let label = label
        let consent = consent
        guard await value({ try await $0.enroll(localWalletId: localWalletId, label: label, consent: consent) }) != nil else { return }
        self.consent = false
        await load()
    }

    func open(_ id: String) async {
        snapshot = nil
        scan = nil
        await refresh(id)
    }

    func refresh(_ id: String) async {
        guard let localWalletId else { error = Localized.phaseoneHdSelectWallet; return }
        guard let snapshot = await value({ try await $0.snapshot(id: id, localWalletId: localWalletId) }) else { return }
        self.snapshot = snapshot
        if let scanId = snapshot.lastScanId {
            scan = await value { try await $0.scanStatus(id: scanId) }
        }
    }

    func allocate(_ id: String) async {
        guard let localWalletId else { error = Localized.phaseoneHdSelectWallet; return }
        if await value({ try await $0.allocate(id: id, localWalletId: localWalletId) }) != nil {
            await refresh(id)
        }
    }

    func startScan(_ id: String) async {
        guard let gap = UInt32(gapLimit), let maximum = UInt32(maximumIndex) else { error = Localized.phaseoneHdScanRange; return }
        if let scan = await value({ try await $0.startScan(id: id, gapLimit: gap, maxIndex: maximum) }) {
            self.scan = scan
        }
    }

    func refreshScan() async {
        guard let scan else { return }
        if let scan = await value({ try await $0.scanStatus(id: scan.id) }) {
            self.scan = scan
        }
    }

    func cancelScan() async {
        guard let scan else { return }
        if let scan = await value({ try await $0.cancelScan(id: scan.id) }) {
            self.scan = scan
        }
    }

    func resumeScan() async {
        guard let scan, let maximum = UInt32(maximumIndex) else { error = Localized.phaseoneHdScanRange; return }
        if let scan = await value({ try await $0.resumeScan(id: scan.id, maxIndex: maximum) }) {
            self.scan = scan
        }
    }

    public func cancel() {
        revision &+= 1
        consent = false
        snapshot = nil
        scan = nil
        if isBusy {
            do { try service.cancelWorkflow() }
            catch { self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError }
        }
    }

    public func clearMemory() {
        error = nil
        cancel()
        wallets = []
        accounts = []
        localWalletId = nil
        label = ""
    }

    private func value<T: Sendable>(_ call: @escaping @Sendable (any BsvHomePlatformHdServiceProtocol) async throws -> T) async -> T? {
        guard !isBusy else { return nil }
        isBusy = true
        error = nil
        let current = revision
        let service = service
        defer { isBusy = false }
        do {
            let result = try await Task.detached(priority: .userInitiated) { try await call(service) }.value
            guard current == revision else { return nil }
            return result
        } catch {
            if let accountError = error as? BsvHomePlatformAccountError, accountError == .LoginRequired || accountError == .SessionChanged {
                if current == revision {
                    clearMemory()
                }
                await onSessionInvalidated()
                return nil
            }
            guard current == revision else { return nil }
            self.error = (error as? BsvHomePlatformAccountError)?.phaseOneMessage ?? Localized.phaseoneServiceError
            return nil
        }
    }
}
