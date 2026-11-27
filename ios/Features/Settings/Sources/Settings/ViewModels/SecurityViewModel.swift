// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Foundation
import BsvHomeCoreServices
import Localization
import Primitives

@Observable
@MainActor
public final class SecurityViewModel {
    public enum PasswordManagement: Hashable, Sendable {
        case online
        case offline
    }

    private let onManagePassword: (@MainActor (PasswordManagement) -> Void)?

    var passwordManagementActions: [PasswordManagement] {
        onManagePassword == nil ? [] : [.online, .offline]
    }

    func managePassword(_ action: PasswordManagement) {
        guard passwordManagementActions.contains(action) else { return }
        onManagePassword?(action)
    }

    private let service: any BiometryAuthenticatable
    private let preferences: ObservablePreferences

    static let reason: String = Localized.Settings.Security.authentication

    var isPresentingAlertMessage: AlertMessage?
    var isEnabled: Bool
    var isPrivacyLockEnabled: Bool
    var isHideBalanceEnabled: Bool {
        get {
            preferences.isHideBalanceEnabled
        }
        set {
            preferences.isHideBalanceEnabled = newValue
        }
    }

    var lockPeriod: LockPeriod {
        didSet { updateLockPeriod() }
    }

    var allLockPeriods: [LockPeriod] {
        LockPeriod.allCases
    }

    public init(
        service: any BiometryAuthenticatable,
        preferences: ObservablePreferences,
        onManagePassword: (@MainActor (PasswordManagement) -> Void)? = nil,
    ) {
        self.service = service
        self.preferences = preferences
        self.onManagePassword = onManagePassword

        lockPeriod = service.lockPeriod
        isEnabled = service.requiresAuthentication
        isPrivacyLockEnabled = service.isPrivacyLockEnabled
    }

    var title: String {
        Localized.Settings.security
    }

    var errorTitle: String {
        Localized.Errors.errorOccurred
    }

    var privacyLockTitle: String {
        Localized.Lock.privacyLock
    }

    var hideBalanceTitle: String {
        Localized.Settings.hideBalance
    }

    var lockPeriodTitle: String {
        Localized.Lock.requireAuthentication
    }

    var authenticationFooter: String {
        Localized.Lock.footer
    }

    var authenticationTitle: String {
        switch service.availableAuthentication {
        case .biometrics: Localized.Settings.enableValue("Face ID")
        case .passcode, .none: Localized.Settings.enablePasscode
        }
    }
}

// MARK: - Business Logic

extension SecurityViewModel {
    func toggleBiometrics() async {
        guard isEnabled != service.requiresAuthentication else { return }
        do {
            try await service.enableAuthentication(isEnabled, reason: SecurityViewModel.reason)
            isPrivacyLockEnabled = service.isPrivacyLockEnabled
            lockPeriod = service.lockPeriod
        } catch let error as BiometryAuthenticationError {
            if !error.isAuthenticationCancelled {
                isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
            }
            isEnabled.toggle()
        } catch {
            isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
            isEnabled.toggle()
        }
    }

    func togglePrivacyLock() {
        guard isPrivacyLockEnabled != service.isPrivacyLockEnabled else { return }
        do {
            try service.togglePrivacyLock(enabled: isPrivacyLockEnabled)
        } catch {
            isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
            isPrivacyLockEnabled.toggle()
        }
    }

    func updateLockPeriod() {
        do {
            try service.update(period: lockPeriod)
        } catch {
            isPresentingAlertMessage = AlertMessage(message: error.localizedDescription)
            lockPeriod = service.lockPeriod
        }
    }
}
