import Components
import struct BsvHomeCore.BsvHomePlatformEmailChallenge
import Localization
import Style
import SwiftUI

public struct PlatformAccountScene<Wallets: View, Advanced: View>: View {
    @Bindable private var model: PlatformAccountViewModel
    private let wallets: () -> Wallets
    private let advanced: () -> Advanced

    public init(model: PlatformAccountViewModel, @ViewBuilder wallets: @escaping () -> Wallets, @ViewBuilder advanced: @escaping () -> Advanced) {
        self.model = model
        self.wallets = wallets
        self.advanced = advanced
    }

    public var body: some View {
        PhaseOnePage(title: Localized.phaseoneAccount, isBusy: model.isBusy, error: model.error) {
            if let profile = model.profile {
                VStack(alignment: .leading, spacing: Spacing.small) {
                    Text(profile.nickname ?? profile.email).font(.title2.weight(.heavy))
                    Text(profile.email).textSelection(.enabled)
                    Text(Localized.phaseoneEmailWalletHint).font(.caption)
                }.wowPayCard()
                NavigationLink { wallets() } label: { Label(Localized.phaseoneMpcWallets, systemImage: "wallet.bifold") }
                NavigationLink(Localized.phaseoneProfile, value: PlatformAccountRoute.profile)
                NavigationLink(Localized.phaseonePasskey, value: PlatformAccountRoute.passkeys)
                NavigationLink(Localized.phaseoneDevices, value: PlatformAccountRoute.devices)
                NavigationLink(Localized.phaseoneChangeEmail, value: PlatformAccountRoute.email)
                advanced()
                NavigationLink(Localized.phaseoneClosure, value: PlatformAccountRoute.closure)
                Button(Localized.phaseoneRenew) { Task { await model.renew() } }
                Button(Localized.phaseoneSignOut, role: .destructive) { Task { await model.logout() } }
            } else {
                login
                NavigationLink(Localized.phaseoneReopen, value: PlatformAccountRoute.reopen)
                advanced()
            }
        }
        .disabled(model.isBusy)
        .task { await model.load() }
        .refreshable { await model.load() }
        .navigationDestination(for: PlatformAccountRoute.self) { route in
            page(route)
        }
    }

    private var login: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(Localized.phaseoneEmailWalletHint).font(.caption)
            TextField(Localized.phaseoneEmail, text: $model.email)
                .textContentType(.emailAddress).keyboardType(.emailAddress).textInputAutocapitalization(.never).autocorrectionDisabled()
                .wowPayInput()
            Button(Localized.phaseoneSendCode) { Task { await model.requestCode() } }
                .buttonStyle(.wowPaySecondary())
            if let challenge = model.challenge {
                challengeDetails(challenge)
                codeField
                Button(Localized.phaseoneLogin) { Task { await model.loginEmail() } }.buttonStyle(.wowPay())
            }
            Button(Localized.phaseonePasskey) { Task { await model.loginPasskey() } }
                .buttonStyle(.wowPay())
                .accessibilityIdentifier("phaseOnePasskeyLogin")
        }
    }

    private func page(_ route: PlatformAccountRoute) -> some View {
        PhaseOnePage(title: title(route), isBusy: model.isBusy, error: model.error) {
            switch route {
            case .profile:
                TextField(Localized.phaseoneLabel, text: $model.nickname).wowPayInput()
                Button(Localized.Common.save) { Task { await model.saveProfile() } }.buttonStyle(.wowPay())
            case .passkeys:
                TextField(Localized.phaseoneLabel, text: $model.label).wowPayInput()
                Button(Localized.phaseoneAddPasskey) { Task { await model.addPasskey() } }.buttonStyle(.wowPay())
                ForEach(model.passkeys, id: \.id) { passkey in
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text(passkey.label).font(.callout.bold())
                        Text(passkey.createdAt).font(.caption)
                        Button(Localized.phaseoneRevoke, role: .destructive) { Task { await model.revokePasskey(passkey.id) } }
                            .disabled(!model.canRevokePasskey)
                    }.wowPayCard()
                }
            case .devices:
                TextField(Localized.phaseoneReason, text: $model.reason, axis: .vertical).wowPayInput()
                ForEach(model.devices, id: \.id) { device in
                    VStack(alignment: .leading, spacing: Spacing.small) {
                        Text(device.deviceType).font(.callout.bold())
                        Text(device.id).font(.caption).textSelection(.enabled)
                        Text(device.lastSeenAt ?? "—").font(.caption)
                        Button(Localized.phaseoneRevoke, role: .destructive) { Task { await model.revokeDevice(device.id) } }
                    }.wowPayCard()
                }
            case .closure:
                closure
            case .email:
                TextField(Localized.phaseoneEmail, text: $model.email).keyboardType(.emailAddress).textInputAutocapitalization(.never).autocorrectionDisabled().wowPayInput()
                Button(Localized.phaseoneSendCode) { Task { await model.changeEmailStart() } }.buttonStyle(.wowPaySecondary())
                if let challenge = model.challenge {
                    challengeDetails(challenge)
                    codeField
                    Button(Localized.Common.continue) { Task { await model.changeEmailFinish() } }.buttonStyle(.wowPay())
                }
            case .reopen:
                Text(Localized.phaseoneClosureHint).font(.body.weight(.medium))
                Button(Localized.phaseonePasskey) { Task { await model.reopen() } }.buttonStyle(.wowPay())
                if let challenge = model.challenge {
                    challengeDetails(challenge)
                    codeField
                    Button(Localized.Common.continue) { Task { await model.loginEmail(reopen: true) } }.buttonStyle(.wowPay())
                }
            }
        }
        .disabled(model.isBusy)
        .task {
            model.clearInput(); if route == .closure {
                await model.closureStatus()
            }
        }
    }

    private var closure: some View {
        VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
            Text(Localized.phaseoneClosureHint)
            if let closure = model.closure {
                Text(closure.id).font(.caption).textSelection(.enabled)
                Text(Localized.phaseoneStatus + ": " + String(describing: closure.status))
                Text(closure.readyAt ?? "—").font(.caption)
            }
            Toggle(Localized.phaseoneAckAddress, isOn: $model.acknowledgeAddress)
            Toggle(Localized.phaseoneAckRetention, isOn: $model.acknowledgeRetention)
            Button(Localized.phaseoneRequestClosure) { Task { await model.requestClosure() } }
                .buttonStyle(.wowPaySecondary())
                .disabled(!model.acknowledgeAddress || !model.acknowledgeRetention)
            if model.closure != nil {
                Button(Localized.Common.cancel) { Task { await model.cancelClosure() } }.buttonStyle(.wowPaySecondary())
                Button(Localized.phaseoneFinishClosure, role: .destructive) { Task { await model.finishClosure() } }
                    .disabled(!model.acknowledgeAddress || !model.acknowledgeRetention)
            }
        }
    }

    private var codeField: some View {
        TextField(Localized.phaseoneCode, text: $model.code)
            .textContentType(.oneTimeCode).keyboardType(.numberPad).wowPayInput()
    }

    private func challengeDetails(_ challenge: BsvHomePlatformEmailChallenge) -> some View {
        VStack(alignment: .leading, spacing: Spacing.small) {
            LabeledContent(Localized.Common.expiration, value: challenge.expiresAt)
            if let seconds = challenge.retryAfterSeconds {
                Text(Localized.phaseoneRetryAfter(Int(seconds)))
            }
        }.font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
    }

    private func title(_ route: PlatformAccountRoute) -> String {
        switch route {
        case .profile: Localized.phaseoneProfile
        case .passkeys: Localized.phaseonePasskey
        case .devices: Localized.phaseoneDevices
        case .closure: Localized.phaseoneClosure
        case .email: Localized.phaseoneChangeEmail
        case .reopen: Localized.phaseoneReopen
        }
    }
}
