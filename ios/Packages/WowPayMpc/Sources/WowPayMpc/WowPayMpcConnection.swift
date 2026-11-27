import Foundation
import Keychain
import WowPayMpcFFI

final class WowPayMpcConnection: @unchecked Sendable {
    private let queue = DispatchQueue(label: "com.wowpay.mpc.native", qos: .userInitiated)
    private var handle: UInt64 = 0
    private var storage: WowPayMpcStorage?

    deinit {
        let handle = handle
        queue.async {
            let result = wowpay_mpc_close(handle)
            if result.buffer_id != 0 {
                _ = wowpay_mpc_buffer_free(result.buffer_id)
            }
        }
    }

    func open(walletId: String, namespace: String, keychain: any Keychain, rootURL: URL?) async throws {
        try await onQueue {
            guard wowpay_mpc_api_version() == 1 else { throw WowPayMpcError(code: "NATIVE_HANDSHAKE_INVALID") }
            let storage = try WowPayMpcStorage(walletId: walletId, namespace: namespace, keychain: keychain, rootURL: rootURL)
            let result = try storage.withSealKey { key in
                let path = Array(storage.directory.path.utf8)
                let wallet = Array(walletId.utf8)
                return path.withUnsafeBufferPointer { path in
                    wallet.withUnsafeBufferPointer { wallet in
                        key.withUnsafeBytes { key in
                            wowpay_mpc_open(path.baseAddress, path.count, wallet.baseAddress, wallet.count, key.bindMemory(to: UInt8.self).baseAddress, key.count)
                        }
                    }
                }
            }
            _ = try self.response(result)
            guard result.handle != 0 else { throw WowPayMpcError(code: "NATIVE_HANDLE_INVALID") }
            self.handle = result.handle
            self.storage = storage
            do {
                try storage.protectFiles()
            } catch {
                let closed = wowpay_mpc_close(self.handle)
                if closed.buffer_id != 0 {
                    _ = wowpay_mpc_buffer_free(closed.buffer_id)
                }
                self.handle = 0
                throw WowPayMpcError(code: "MPC_STORAGE_PROTECTION_UNAVAILABLE")
            }
        }
    }

    func exchange(_ commandJSON: String) async throws -> String {
        try await onQueue {
            guard self.handle != 0 else { throw WowPayMpcError(code: "MPC_PARTICIPANT_CLOSED") }
            var command = Data(commandJSON.utf8)
            defer { command.resetBytes(in: command.startIndex ..< command.endIndex) }
            let result = command.withUnsafeBytes { bytes in
                wowpay_mpc_exchange(self.handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
            }
            guard let response = try self.response(result),
                  let bytes = response.data(using: .utf8),
                  let value = try? JSONSerialization.jsonObject(with: bytes, options: [.fragmentsAllowed])
            else {
                throw WowPayMpcError(code: "NATIVE_RESPONSE_INVALID")
            }
            try self.storage?.protectFiles()
            if let object = value as? [String: Any], let code = object["error"] as? String {
                throw WowPayMpcError(code: code)
            }
            return response
        }
    }

    func close() async throws {
        try await onQueue {
            let handle = self.handle
            self.handle = 0
            self.storage = nil
            _ = try self.response(wowpay_mpc_close(handle))
        }
    }

    private func response(_ result: WowPayMpcResult) throws -> String? {
        defer {
            if result.buffer_id != 0 {
                _ = wowpay_mpc_buffer_free(result.buffer_id)
            }
        }
        guard result.length <= 8_000_000 else { throw WowPayMpcError(code: "NATIVE_RESPONSE_INVALID") }
        let text: String?
        if result.length > 0, let bytes = result.data {
            text = String(bytes: UnsafeBufferPointer(start: bytes, count: result.length), encoding: .utf8)
            guard text != nil else { throw WowPayMpcError(code: "NATIVE_RESPONSE_INVALID") }
        } else {
            text = nil
        }
        guard result.status == 0 else {
            throw WowPayMpcError(code: text ?? "NATIVE_STATE_UNAVAILABLE")
        }
        return text
    }

    private func onQueue<T: Sendable>(_ action: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async {
                continuation.resume(with: Result(catching: action))
            }
        }
    }
}
