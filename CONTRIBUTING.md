# Contributing to Cantor

Thank you for helping. Before your first pull request is merged, please read
and agree to the Contributor Licence Agreement below. Say so in the pull
request: "I agree to the Cantor CLA, version 1". Do this once.

## Why there is a CLA

Cantor is AGPL-3.0, with an extra permission that lets it ship through app
stores (`LICENSE-EXCEPTIONS.md`). Only the people who hold the copyright in
the code can grant that permission. Without a CLA, the first contribution
without the permission would stop Cantor from being on the App Store.

Cantor may also later be maintained by a Swedish sole trader (enskild firma)
or a company. That business may sell services around Cantor, such as a
central server for choirs and bands. The CLA lets the copyright holder
transfer the project to that business without asking every contributor
again.

The CLA does **not** take the AGPL away from you or anyone else. Everything
you contribute stays available under the AGPL.

## Contributor Licence Agreement, version 1

"Maintainer" means Hans Hård. From the time he transfers Cantor to a business
he owns or controls, such as an enskild firma or an aktiebolag, it means that
business, with a notice in this file.

For any contribution you submit to Cantor (code, documentation, tests,
translations, images or other material):

1. **Licence to the Maintainer.** You grant the Maintainer a perpetual,
   worldwide, non-exclusive, royalty-free, irrevocable licence to use, copy,
   modify, publish, distribute and sublicense your contribution, under any
   licence terms, including the AGPL-3.0 and its additional permissions.
2. **Your licence to everyone.** You license your contribution to everyone
   under the AGPL-3.0, with the additional permissions in
   `LICENSE-EXCEPTIONS.md`.
3. **Patents.** You grant the Maintainer and all recipients of Cantor a
   perpetual, worldwide, royalty-free licence under any patent claims you
   control that your contribution necessarily infringes, as in section 11 of
   the AGPL-3.0.
4. **Promise to keep it open.** The Maintainer will keep every version that
   contains your contribution available under the AGPL-3.0 (with or without
   the app store permission).
5. **Your right to contribute.** The contribution is your own work, or you
   have the right to submit it under these terms (for example your employer
   agreed). You will say so if any part comes from someone else, and under
   what licence.
6. **Transfer.** The Maintainer may transfer this agreement to a business
   that takes over maintaining Cantor. The promise in point 4 goes with it.
7. **No other obligations.** You give no warranty, and the Maintainer does not
   have to use your contribution.

This CLA follows the common "inbound licence plus promise" pattern. It was
written without a lawyer, and a legal review is planned (`docs/ROADMAP.md`,
phase L). Until that review, contributions are welcome, but the maintainer
may ask you to confirm a revised version.

## Practical rules

- Before a commit: `cargo fmt --all`, `cargo clippy --workspace --all-targets
  -- -D warnings`, and the tests (see `README.md`).
- Name tests after the requirement they cover (`lib_03_…`, `set_01_…`).
- New dependencies must pass `tools/licenses.py`. It fails on any licence
  that is not on the allow-list.
- Never commit sheet music you do not have the rights to publish. The `scores/`
  folder is private and ignored.
