import Components
import Style
import SwiftUI

public struct WowPayAssetDetailView<AssetHeader: View, Content: View>: View {
    @Binding private var selectedTab: WowPayAssetDetailTab
    private let visibleTabs: [WowPayAssetDetailTab]
    private let tabTitle: (WowPayAssetDetailTab) -> String
    private let tabBadge: (WowPayAssetDetailTab) -> Int?
    private let tabLayout: WowPayAssetDetailTabLayout
    private let assetHeader: AssetHeader
    private let content: Content
    private let navigationBarVisibility: Visibility

    public init(selectedTab: Binding<WowPayAssetDetailTab>, tabs: [WowPayAssetDetailTab] = [.overview, .address, .transactions, .about], tabTitle: @escaping (WowPayAssetDetailTab) -> String = { $0.title }, tabBadge: @escaping (WowPayAssetDetailTab) -> Int? = { _ in nil }, tabLayout: WowPayAssetDetailTabLayout = .scrollable, navigationBarVisibility: Visibility = .visible, @ViewBuilder assetHeader: () -> AssetHeader, @ViewBuilder content: () -> Content) {
        _selectedTab = selectedTab
        visibleTabs = tabs
        self.tabTitle = tabTitle
        self.tabBadge = tabBadge
        self.tabLayout = tabLayout
        self.assetHeader = assetHeader()
        self.content = content()
        self.navigationBarVisibility = navigationBarVisibility
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.space16) {
                VStack(alignment: .leading, spacing: Spacing.space12) {
                    assetHeader
                    tabs
                }
                content
            }
            .padding(.horizontal, Spacing.wowPay.page)
            .padding(.top, navigationBarVisibility == .hidden ? .zero : Spacing.wowPay.page)
            .padding(.bottom, Spacing.wowPay.section)
        }
        .wowPayBackground(.solid)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(navigationBarVisibility, for: .navigationBar)
        .toolbar(.hidden, for: .tabBar)
    }

    private var tabs: some View {
        Group {
            switch tabLayout {
            case .scrollable:
                ScrollView(.horizontal, showsIndicators: false) {
                    tabRow
                }
            case .equalWidth:
                ViewThatFits(in: .horizontal) {
                    tabRow
                    ScrollView(.horizontal, showsIndicators: false) {
                        tabRow.fixedSize(horizontal: true, vertical: false)
                    }
                }
            }
        }
        .overlay(alignment: .bottom) {
            Colors.WowPay.border.frame(height: Spacing.space1)
        }
    }

    private var tabRow: some View {
        HStack(spacing: tabLayout == .equalWidth ? .zero : Spacing.wowPay.detailTabGap) {
            ForEach(visibleTabs) { tab in
                Button {
                    selectedTab = tab
                } label: {
                    HStack(spacing: Spacing.space4) {
                        Text(tabTitle(tab))
                            .font(tabLayout == .equalWidth ? .wowPay.multisigTab : .wowPay.label)
                            .fixedSize(horizontal: true, vertical: false)
                        if let count = tabBadge(tab) {
                            WowPayCountBadge(count: count)
                        }
                    }
                    .foregroundStyle(selectedTab == tab ? Colors.WowPay.brand : Colors.WowPay.secondaryText)
                    .padding(.top, tabLayout == .equalWidth ? Spacing.space10 : .zero)
                    .padding(.bottom, tabLayout == .equalWidth ? Spacing.space16 : Spacing.space10)
                    .frame(maxWidth: tabLayout == .equalWidth ? .infinity : nil)
                    .overlay(alignment: .bottom) {
                        if selectedTab == tab, tabLayout == .equalWidth {
                            Colors.WowPay.brand.frame(height: Sizing.wowPay.detailTabUnderline)
                        }
                    }
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("wowPayAssetTab_" + tab.id)
                .accessibilityAddTraits(selectedTab == tab ? .isSelected : [])
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
