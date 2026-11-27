# Localization

Use this skill for shared localization flow and generated output locations.

- From the repository root, generate only iOS localization output:
  ```bash
  just ios localize
  ```
- Mobile app localization source of truth lives in `localization/app/*.ftl`
- Use Fluent message IDs with underscores, for example `common_cancel`
- Fluent comments are supported (`#`, `##`, `###`) and ignored by generation; put source context comments in English `en.ftl`
- Use native format placeholders such as `%@` and `%d`, preserving their count and types in every language. The app generator rejects Fluent placeables such as `{ $asset }`.
- Add new keys in the matching prefix section (`common_*` under `# Common`, `wallet_*` under `# Wallet`, etc.)
- Add each new app key to every language file, translated for the context where the string is used
- iOS InfoPlist localization source lives in `localization/InfoPlist/*.ftl`
- iOS widget localization source lives in `localization/widget/*.ftl`
- iOS localization output lives under `ios/Packages/Localization/`: one `.xcstrings` String Catalog per table (app, InfoPlist, widget) plus the typed accessors `Localized.swift` and `WidgetLocalized.swift`, all written by the generator
- Treat generated localization outputs as generated artifacts, not hand-edited source
- Never resolve merge conflicts in generated localization outputs by hand: rebase, keep your side of the tree, run `just ios localize`, and include the regenerated iOS files
- Core/backend localization is separate; see [core/skills/localization.md](../core/skills/localization.md)
