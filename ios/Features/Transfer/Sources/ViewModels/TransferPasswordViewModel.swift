// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import protocol BsvHomeCore.BsvHomePortalTransferPasswordServiceProtocol
import enum BsvHomeCore.BsvHomeTransferPasswordError
import struct BsvHomeCore.BsvHomeTransferPasswordSnapshot
import BsvHomeCorePrimitives
import Localization
import PrimitivesComponents

@Observable
@MainActor
public final class TransferPasswordViewModel: Identifiable {
    public let id = UUID()
    public let mode: TransferPasswordMode
    public let walletName: String

    var currentPassword = ""
    var newPassword = ""
    var confirmation = ""
    private(set) var state: StateViewType<BsvHomeTransferPasswordSnapshot> = .noData
    private(set) var isWorking = false
    enum Outcome { case pending, completed, cancelled }

    private(set) var outcome: Outcome = .pending
    var isComplete: Bool { outcome == .completed }
    var isCancelled: Bool { outcome == .cancelled }
    private(set) var errorMessage: String?

    private let backend: TransferPasswordBackend
    private let onComplete: @MainActor () -> Void
    private let onCancel: @MainActor () -> Void
    private var service: TransferPasswordBackend.Service?
    private(set) var loadingTask: Task<Void, Never>?
    private(set) var submission: Task<Void, Never>?
    private var revision: UInt = 0
    private var isFinished: Bool { outcome != .pending }

    public convenience init(
        mode: TransferPasswordMode,
        walletName: String,
        makeService: @escaping @MainActor () throws -> any BsvHomePortalTransferPasswordServiceProtocol,
        onComplete: @escaping @MainActor () -> Void = {},
        onCancel: @escaping @MainActor () -> Void = {},
    ) {
        self.init(mode: mode, walletName: walletName, backend: .online(makeService), onComplete: onComplete, onCancel: onCancel)
    }

    public init(
        mode: TransferPasswordMode,
        walletName: String,
        backend: TransferPasswordBackend,
        onComplete: @escaping @MainActor () -> Void = {},
        onCancel: @escaping @MainActor () -> Void = {},
    ) {
        self.mode = mode
        self.walletName = walletName
        self.backend = backend
        self.onComplete = onComplete
        self.onCancel = onCancel
    }

    var title: String {
        backend.title
    }

    func errorDescription(_ error: any Error) -> String {
        backend.errorDescription(error)
    }

    var hint: String { showsNewPassword ? Localized.wowpayTransferPasswordSetupHint : Localized.wowpayTransferPasswordVerifyHint }
    var buttonTitle: String { showsNewPassword ? Localized.Common.save : Localized.Common.continue }

    var showsCurrentPassword: Bool { state.value?.configured == true }
    var showsNewPassword: Bool { mode == .management || state.value?.configured == false }
    var currentPasswordTitle: String {
        mode == .management ? Localized.wowpayTransferPasswordCurrent : Localized.wowpayTransferPasswordInput
    }

    var canSubmit: Bool {
        guard !isWorking, !isFinished, state.value != nil else { return false }
        return (!showsCurrentPassword || !currentPassword.isEmpty) && (!showsNewPassword || (!newPassword.isEmpty && !confirmation.isEmpty))
    }

    func startLoading() {
        guard case .noData = state else { return }
        reloadStatus()
    }

    func reloadStatus() {
        guard !isFinished, !isWorking, loadingTask == nil else { return }
        revision &+= 1
        let currentRevision = revision
        clearPasswords()
        state = .loading
        errorMessage = nil
        loadingTask = Task { [weak self] in
            await self?.fetchStatus(revision: currentRevision)
        }
    }

    func load() async {
        reloadStatus()
        await loadingTask?.value
    }

    private func fetchStatus(revision currentRevision: UInt) async {
        guard currentRevision == revision, !isFinished, !Task.isCancelled else { return }
        defer {
            if currentRevision == revision {
                loadingTask = nil
            }
        }
        do {
            let service = try service ?? backend.makeService()
            self.service = service
            guard currentRevision == revision, !isFinished, !Task.isCancelled else {
                service.cancel()
                return
            }
            let snapshot = try await service.status()
            guard currentRevision == revision, !isFinished, !Task.isCancelled else { return }
            state = .data(snapshot)
        } catch {
            guard currentRevision == revision, !isFinished, !Task.isCancelled else { return }
            if error.isCancelled {
                cancel()
            } else {
                state = .error(error as? BsvHomeTransferPasswordError ?? .Unavailable)
            }
        }
    }

    func submit() {
        guard canSubmit, let snapshot = state.value, let service else { return }
        revision &+= 1
        let currentRevision = revision
        let currentPassword = currentPassword
        let newPassword = newPassword
        let confirmation = confirmation
        clearPasswords()
        errorMessage = nil
        isWorking = true
        submission = Task { [weak self] in
            guard let self, currentRevision == revision, !isFinished, !Task.isCancelled else { return }
            do {
                switch mode {
                case .authorization where snapshot.configured:
                    try await service.verify(snapshot: snapshot, password: currentPassword)
                case .authorization, .management:
                    _ = try await service.set(snapshot: snapshot, currentPassword: snapshot.configured ? currentPassword : nil, newPassword: newPassword, confirmation: confirmation)
                }
                guard currentRevision == revision, !isFinished, !Task.isCancelled else { return }
                isWorking = false
                submission = nil
                outcome = .completed
                onComplete()
            } catch {
                guard currentRevision == revision, !isFinished, !Task.isCancelled else { return }
                isWorking = false
                submission = nil
                if error.isCancelled {
                    cancel()
                } else {
                    let error = error as? BsvHomeTransferPasswordError ?? .Unavailable
                    if error.requiresStatusReload() {
                        state = .error(error)
                    } else {
                        errorMessage = backend.errorDescription(error)
                    }
                }
            }
        }
    }

    public func cancel() {
        guard !isFinished else { return }
        outcome = .cancelled
        revision &+= 1
        service?.cancel()
        loadingTask?.cancel()
        loadingTask = nil
        submission?.cancel()
        submission = nil
        clearPasswords()
        isWorking = false
        onCancel()
    }

    private func clearPasswords() {
        currentPassword = ""
        newPassword = ""
        confirmation = ""
    }
}
