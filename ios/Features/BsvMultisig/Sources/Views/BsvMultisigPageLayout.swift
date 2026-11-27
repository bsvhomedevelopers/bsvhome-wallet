import Style
import SwiftUI

struct BsvMultisigPageLayout<Content: View>: View {
    let title: String
    let subtitle: String
    let navigationBarVisibility: Visibility
    @ViewBuilder let content: () -> Content

    init(title: String, subtitle: String = "", navigationBarVisibility: Visibility = .visible, @ViewBuilder content: @escaping () -> Content) {
        self.title = title
        self.subtitle = subtitle
        self.navigationBarVisibility = navigationBarVisibility
        self.content = content
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.wowPay.card) {
                if !subtitle.isEmpty {
                    Text(subtitle).font(.caption).foregroundStyle(Colors.WowPay.secondaryText)
                }
                content()
            }
            .foregroundStyle(Colors.WowPay.heading)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.wowPay.page)
        }
        .wowPayBackground(.solid)
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(navigationBarVisibility, for: .navigationBar)
    }
}
