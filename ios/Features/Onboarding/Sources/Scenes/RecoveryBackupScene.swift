import Localization
import PrimitivesComponents
import Style
import SwiftUI

struct RecoveryBackupScene: View {
    let recovery: String

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.large) {
                Text(Localized.bsvHdBackupHint)
                Text(recovery)
                    .font(.body.monospaced())
                    .textSelection(.disabled)
                    .privacySensitive()
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground()
        .navigationTitle(Localized.bsvHdBackupTitle)
        .protectFromScreenRecording()
    }
}
