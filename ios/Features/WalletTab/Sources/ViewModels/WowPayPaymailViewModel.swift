import Foundation
import struct BsvHomeCore.BsvHomePaymailBinding
import enum BsvHomeCore.BsvHomePaymailError
import protocol BsvHomeCore.BsvHomePaymailServiceProtocol
import struct BsvHomeCore.BsvHomePaymailSnapshot
import Localization
import Observation
import PrimitivesComponents

@MainActor
@Observable
public final class WowPayPaymailViewModel {
    enum State {
        case idle
        case loading
        case loaded(BsvHomePaymailSnapshot)
        case failed(String)
    }

    enum Operation: Equatable {
        case idle
        case submitting
        case pendingVerification
    }

    struct Confirmation: Identifiable {
        let id = UUID()
        let snapshot: BsvHomePaymailSnapshot
        let alias: String
        let domain: String
        let active: Bool

        var handle: String { "\(alias)@\(domain)" }
        var title: String { active ? Localized.paymailEnable : Localized.paymailDisable }
    }

    private let walletId: String
    private let makeService: @Sendable () throws -> any BsvHomePaymailServiceProtocol
    private var service: (any BsvHomePaymailServiceProtocol)?
    private var revision = UUID()
    private(set) var state: State = .idle
    private(set) var operation: Operation = .idle
    var alias = ""
    var domain = ""
    var confirmation: Confirmation?
    private(set) var notice: String?

    public init(walletId: String, makeService: @escaping @Sendable () throws -> any BsvHomePaymailServiceProtocol) {
        self.walletId = walletId
        self.makeService = makeService
    }

    var snapshot: BsvHomePaymailSnapshot? {
        if case let .loaded(snapshot) = state {
            return snapshot
        }
        return nil
    }

    public var activeHandle: String? { snapshot?.activeHandle }
    public var receiveTitle: String {
        if let activeHandle {
            return activeHandle
        }
        switch state {
        case .idle, .loading: return Localized.Common.loading
        case .failed: return Localized.paymailUnavailable
        case .loaded: return Localized.wowpayPaymailSetup
        }
    }

    var isBusy: Bool { operation == .submitting }

    public func load() async {
        guard !isBusy else { return }
        let revision = UUID()
        self.revision = revision
        state = .loading
        confirmation = nil
        notice = nil
        do {
            let service = try service ?? makeService()
            self.service = service
            let snapshot = try await service.load()
            try Task.checkCancellation()
            guard self.revision == revision else { return }
            guard snapshot.walletId == walletId else {
                state = .failed(Localized.paymailBindingChanged)
                return
            }
            state = .loaded(snapshot)
            switch snapshot.verification {
            case .clear, .reconciled:
                operation = .idle
            case .pending:
                operation = .pendingVerification
                notice = Localized.paymailPendingVerification
            case .changed:
                operation = .idle
                notice = Localized.paymailBindingChanged
            }
            if !snapshot.registrationDomains.contains(domain) {
                domain = snapshot.registrationDomains.first ?? ""
            }
        } catch {
            guard self.revision == revision else { return }
            state = .failed(error.localizedDescription)
        }
    }

    func confirmRegistration() {
        guard let snapshot, operation == .idle else { return }
        confirmation = Confirmation(
            snapshot: snapshot,
            alias: alias.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
            domain: domain,
            active: true,
        )
    }

    func confirmActivity(_ binding: BsvHomePaymailBinding) {
        guard let snapshot, operation == .idle, binding.availability == .available else { return }
        let parts = binding.handle.split(separator: "@")
        confirmation = Confirmation(snapshot: snapshot, alias: String(parts[0]), domain: String(parts[1]), active: !binding.active)
    }

    func submit(_ confirmation: Confirmation) async {
        guard let service, operation == .idle, snapshot == confirmation.snapshot else { return }
        let revision = revision
        self.confirmation = nil
        operation = .submitting
        notice = nil
        do {
            let intent = try await service.bindingIntent(snapshot: confirmation.snapshot, alias: confirmation.alias, domain: confirmation.domain, active: confirmation.active)
            try Task.checkCancellation()
            guard self.revision == revision else { return }
            let result = try await service.submitBinding(intent: intent)
            try Task.checkCancellation()
            guard self.revision == revision else { return }
            if let snapshot = result.snapshot {
                state = .loaded(snapshot)
            }
            switch result.status {
            case .confirmed, .reconciled:
                operation = .idle
                alias = ""
            case .pendingVerification:
                operation = .pendingVerification
                notice = Localized.paymailPendingVerification
                if result.snapshot == nil {
                    state = .failed(Localized.paymailPendingVerification)
                }
            case .changed:
                operation = .idle
                notice = Localized.paymailBindingChanged
            }
        } catch {
            guard self.revision == revision else { return }
            operation = error as? BsvHomePaymailError == .PendingVerification ? .pendingVerification : .idle
            notice = error.localizedDescription
        }
    }

    public func cancel() {
        revision = UUID()
        service?.cancel()
        confirmation = nil
        state = .idle
        operation = .idle
    }
}
