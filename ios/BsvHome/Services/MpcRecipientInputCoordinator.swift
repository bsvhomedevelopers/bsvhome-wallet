import Components
import Foundation
import struct BsvHomeCore.BsvHomePaymentRecipient
import struct BsvHomeCore.BsvHomeSendContactRecipient
import protocol BsvHomeCore.BsvHomeSendServiceProtocol
import BsvHomeCorePrimitives
import Localization
import Observation
import enum Primitives.Chain
import Store

@Observable
@MainActor
final class MpcRecipientInputCoordinator {
    struct Presentation: Identifiable {
        enum Kind { case scanner, contacts }
        let id = UUID()
        let kind: Kind
        let selection: @MainActor @Sendable (BsvHomePaymentRecipient) -> Void
    }

    private enum InputState {
        case idle
        case presented(Presentation)
        case parsing(Presentation)
    }

    let contactsQuery = ObservableQuery(ContactsRequest(chain: .bitcoinSv), initialValue: [])
    private(set) var contacts: StateViewType<[BsvHomeSendContactRecipient]> = .noData
    var error: String?
    private let service: any BsvHomeSendServiceProtocol
    private let asset = Chain.bitcoinSv.asset.map()
    private var state = InputState.idle
    private var contactsRevision: UInt = 0

    init(service: any BsvHomeSendServiceProtocol) {
        self.service = service
    }

    var presentation: Presentation? {
        get {
            if case let .presented(request) = state {
                return request
            }
            return nil
        }
        set {
            if let newValue {
                state = .presented(newValue)
            } else if case .presented = state {
                state = .idle
            }
        }
    }

    func presentScanner(_ selection: @escaping @MainActor @Sendable (BsvHomePaymentRecipient) -> Void) {
        error = nil
        state = .presented(Presentation(kind: .scanner, selection: selection))
    }

    func presentContacts(_ selection: @escaping @MainActor @Sendable (BsvHomePaymentRecipient) -> Void) {
        error = nil
        state = .presented(Presentation(kind: .contacts, selection: selection))
        Task { await refreshContacts() }
    }

    func refreshContacts() async {
        guard case let .presented(request) = state, request.kind == .contacts else { return }
        contactsRevision &+= 1
        let current = contactsRevision
        let source = contactsQuery.value
        let records = source.map { $0.contact.map() }
        let addresses = source.flatMap(\.addresses).map { $0.map() }
        let asset = asset
        let service = service
        contacts = .loading
        do {
            let rows = try await Task.detached(priority: .userInitiated) { try service.contactRecipients(asset: asset, contacts: records, addresses: addresses) }.value
            guard current == contactsRevision, isCurrent(request) else { return }
            contacts = .data(rows)
        } catch {
            guard current == contactsRevision, isCurrent(request) else { return }
            contacts = .error(error)
        }
    }

    func scanned(_ text: String, request: Presentation) {
        guard isCurrent(request) else { return }
        state = .parsing(request)
        let service = service
        let asset = asset
        Task {
            do {
                let recipient = try await Task.detached(priority: .userInitiated) { try service.scan(asset: asset, input: text) }.value
                guard isCurrent(request) else { return }
                state = .idle
                request.selection(recipient)
            } catch {
                guard isCurrent(request) else { return }
                state = .idle
                self.error = Localized.Errors.invalidAssetAddress(asset.name)
            }
        }
    }

    func select(_ row: BsvHomeSendContactRecipient, request: Presentation) {
        guard isCurrent(request) else { return }
        state = .idle
        request.selection(BsvHomePaymentRecipient(recipient: row.recipient, amount: nil))
    }

    func cancel() {
        contactsRevision &+= 1
        state = .idle
        error = nil
    }

    private func isCurrent(_ request: Presentation) -> Bool {
        switch state {
        case .idle: false
        case let .presented(current), let .parsing(current): current.id == request.id
        }
    }
}
