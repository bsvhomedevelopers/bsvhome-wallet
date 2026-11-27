import Localization
import Style
import SwiftUI

struct WalletImportInputField: View {
    let placeholder: String
    let isSecret: Bool
    let isDisabled: Bool
    let identifier: String
    let focus: FocusState<Bool>.Binding
    @Binding var input: String
    @Binding var isSecretVisible: Bool

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.small) {
            Group {
                if isSecret, !isSecretVisible {
                    SecureField(placeholder, text: $input)
                } else {
                    TextField(placeholder, text: $input, axis: .vertical)
                        .lineLimit(8)
                }
            }
            .accessibilityIdentifier(identifier)
            .autocorrectionDisabled(true)
            .textInputAutocapitalization(.never)
            .textContentType(nil)
            .keyboardType(.asciiCapable)
            .frame(minHeight: Sizing.wowPay.secretInputHeight, alignment: .top)
            .focused(focus)
            .disabled(isDisabled)
            .privacySensitive()

            if isSecret {
                Button { isSecretVisible.toggle() } label: {
                    Image(systemName: isSecretVisible ? "eye.slash" : "eye")
                        .frame(minWidth: Sizing.wowPay.minimumTapTarget, minHeight: Sizing.wowPay.minimumTapTarget)
                }
                .accessibilityLabel(isSecretVisible ? Localized.Common.hide : Localized.Common.show(placeholder))
                .disabled(isDisabled)
            }
        }
    }
}
