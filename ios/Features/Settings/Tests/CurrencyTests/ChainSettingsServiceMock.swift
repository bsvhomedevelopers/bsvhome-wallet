import Foundation
import BsvHomeCore

final class ChainSettingsServiceMock: BsvHomeChainSettingsServiceProtocol, @unchecked Sendable {
    private let lock = NSLock()
    private var explorer = "Committed Explorer"
    private var explorerWriteFailure = false
    private var nodeReadFailure = false
    private var nodeSelectionFailure = false
    private var nodeDeletionFailure = false
    private var readFailureAfterMutation = false
    private var selectionResetBeforeDeleteFailure = false
    private var reads = 0
    private var checkedURLs: [String] = []
    private var importedURLs: [String] = []
    private var checkReply: (@Sendable (String) async throws -> BsvHomeNodeCheck)?
    private var addReply: (@Sendable (String) async throws -> Void)?
    private var selections = 0
    private var deletions = 0
    private var records = [
        BsvHomeNodeSelection(url: "https://first.example.invalid", host: "first.example.invalid", isSelected: true),
        BsvHomeNodeSelection(url: "https://second.example.invalid", host: "second.example.invalid", isSelected: false),
    ]

    var rejectExplorerWrites: Bool {
        get { lock.withLock { explorerWriteFailure } }
        set { lock.withLock { explorerWriteFailure = newValue } }
    }

    var rejectNodeReads: Bool {
        get { lock.withLock { nodeReadFailure } }
        set { lock.withLock { nodeReadFailure = newValue } }
    }

    var rejectNodeSelections: Bool {
        get { lock.withLock { nodeSelectionFailure } }
        set { lock.withLock { nodeSelectionFailure = newValue } }
    }

    var rejectNodeDeletions: Bool {
        get { lock.withLock { nodeDeletionFailure } }
        set { lock.withLock { nodeDeletionFailure = newValue } }
    }

    var failReadsAfterMutation: Bool {
        get { lock.withLock { readFailureAfterMutation } }
        set { lock.withLock { readFailureAfterMutation = newValue } }
    }

    var resetSelectionBeforeDeleteFailure: Bool {
        get { lock.withLock { selectionResetBeforeDeleteFailure } }
        set { lock.withLock { selectionResetBeforeDeleteFailure = newValue } }
    }

    var checkedNodeURLs: [String] { lock.withLock { checkedURLs } }
    var addedNodeURLs: [String] { lock.withLock { importedURLs } }

    func setCheckNodeReply(_ reply: @escaping @Sendable (String) async throws -> BsvHomeNodeCheck) {
        lock.withLock { checkReply = reply }
    }

    func setAddNodeReply(_ reply: @escaping @Sendable (String) async throws -> Void) {
        lock.withLock { addReply = reply }
    }

    var nodeReadCount: Int { lock.withLock { reads } }
    var nodeSelectionCount: Int { lock.withLock { selections } }
    var nodeDeletionCount: Int { lock.withLock { deletions } }
    var committedNodes: [BsvHomeNodeSelection] { lock.withLock { records } }

    func setExplorerName(chain _: String, name: String) throws {
        try lock.withLock {
            if explorerWriteFailure {
                throw BsvHomeServiceError.Store(msg: "Rejected test write")
            }
            explorer = name
        }
    }

    func explorerName(chain _: String) -> String {
        lock.withLock { explorer }
    }

    func explorers(chain _: String) -> [String] {
        ["Committed Explorer", "Other Explorer"]
    }

    func chains(query _: String) -> [String] {
        ["bitcoinsv"]
    }

    func nodes(chain _: String) async throws -> [BsvHomeNodeSelection] {
        try lock.withLock {
            reads += 1
            if nodeReadFailure {
                throw BsvHomeServiceError.Store(msg: "Rejected test read")
            }
            return records
        }
    }

    func selectNode(chain _: String, url: String) async throws {
        try lock.withLock {
            selections += 1
            if nodeSelectionFailure {
                throw BsvHomeServiceError.Store(msg: "Rejected test selection")
            }
            records = records.map { node in
                var value = node
                value.isSelected = node.url == url
                return value
            }
            if readFailureAfterMutation {
                nodeReadFailure = true
            }
        }
    }

    func deleteNode(chain _: String, url: String) async throws {
        try lock.withLock {
            deletions += 1
            if nodeDeletionFailure {
                if selectionResetBeforeDeleteFailure {
                    records = records.enumerated().map { index, node in
                        var value = node
                        value.isSelected = index == 0
                        return value
                    }
                }
                throw BsvHomeServiceError.Store(msg: "Rejected test deletion")
            }
            records.removeAll { $0.url == url }
            if readFailureAfterMutation {
                nodeReadFailure = true
            }
        }
    }

    func canDeleteNode(chain _: String, url _: String) -> Bool {
        true
    }

    func nodeFlag(url _: String) -> String? {
        nil
    }

    func nodeStatus(chain _: String, url _: String) async -> BsvHomeNodeStatusState {
        .error
    }

    func nodeCheckDebounceMilliseconds() -> UInt64 {
        0
    }

    func addNode(chain _: String, url: String) async throws {
        let reply = lock.withLock {
            importedURLs.append(url)
            return addReply
        }
        try await reply?(url)
    }

    func checkNode(chain _: String, url: String) async throws -> BsvHomeNodeCheck {
        let reply = lock.withLock {
            checkedURLs.append(url)
            return checkReply
        }
        guard let reply else { throw BsvHomeServiceError.Cancelled }
        return try await reply(url)
    }
}
