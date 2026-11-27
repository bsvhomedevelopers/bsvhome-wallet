import Localization
import Style
import SwiftUI

public struct BsvHdImportSettings: View {
    @Binding private var configuration: BsvHdImportConfiguration
    @Binding private var passphrase: String
    private let isSecret: Bool

    public init(configuration: Binding<BsvHdImportConfiguration>, passphrase: Binding<String>, isSecret: Bool) {
        _configuration = configuration
        _passphrase = passphrase
        self.isSecret = isSecret
    }

    public var body: some View {
        DisclosureGroup(Localized.bsvHdSettings) {
            VStack(alignment: .leading, spacing: Spacing.medium) {
                Text(Localized.bsvHdSettingsHint).font(.footnote)
                Picker(Localized.bsvHdAccountPath, selection: $configuration.pathPreset) {
                    Text(Localized.bsvHdPresetDefault).tag(BsvHdPathPreset.default)
                    Text(Localized.bsvHdPresetWowpay).tag(BsvHdPathPreset.wowPay)
                    Text(Localized.bsvHdPresetSimplycash).tag(BsvHdPathPreset.compatible)
                    Text(Localized.bsvHdPresetCustom).tag(BsvHdPathPreset.custom)
                }
                if configuration.pathPreset == .custom {
                    TextField(Localized.bsvHdAccountPath, text: $configuration.customPath)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                }
                if isSecret {
                    SecureField(Localized.bsvHdPassphrase, text: $passphrase)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                        .privacySensitive()
                    Toggle(Localized.bsvHdLegacy, isOn: $configuration.legacyNonCompliant)
                }
                TextField(Localized.bsvHdExpectedAddress, text: $configuration.expectedAddress)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
            }
            .padding(.top, Spacing.small)
        }
    }
}
