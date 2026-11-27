import Foundation

extension DispatchQueue {
    func asyncTask<T: Sendable>(
        onEnqueued: (@Sendable () -> Void)? = nil,
        execute work: @escaping @Sendable () throws -> T,
    ) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            self.async {
                do {
                    try continuation.resume(returning: work())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
            onEnqueued?()
        }
    }
}
