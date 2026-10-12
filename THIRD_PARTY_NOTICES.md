# Third-party component notices

Zerant includes and depends on software owned by other authors. The [Zerant LICENSE](LICENSE) governs only the original material owned by Zerant's copyright holder. It does **not** relicense upstream dependencies, binaries, designs, data, or code imported from third parties.

The [versioned dependency inventory](third-party-licenses.txt) lists JavaScript packages and Rust registry crates identified from the installed pnpm dependency tree and Cargo metadata. Each entry includes the declared license (or relevant custom license name), package version, project reference, and names of locally available license/notice files.

**Important:** The inventory is a metadata and attribution index—not a complete reproduction of all third-party license texts, a legal clearance opinion, or proof that every transitive binary/asset was audited. Licenses and copyright notices bundled with upstream packages remain authoritative. Before distributing an installable binary, container or SDK, include all license copies, attributions, source-code offers or other materials required by the specific included dependencies.

## Components requiring particular attention

### Native image-processing components

@img/sharp-libvips-linux-x64 declares **LGPL-3.0-or-later**. Its native libvips binary and related packaged components may carry separate notice, relinking and distribution conditions. Review the upstream licenses of the **actual binaries shipped**, not just the JavaScript wrapper.

Project reference: https://sharp.pixelplumbing.com/

### File-level copyleft and data/content licenses

Other dependencies include components with **MPL-2.0** (including some WebAuthn Rust crates, axe-core, and lightningcss), **Unicode-3.0** (ICU4X-related crates), **CC-BY-4.0** (caniuse-lite), and **CDLA-Permissive-2.0** (webpki root certificate data). These are not equivalent to a single permissive MIT license, and any obligations remain attached to their respective materials.

The inventory may contain **build-only, test-only, optional or platform-specific** dependencies. Presence in a lockfile does not prove a particular package is included in every hosted or distributed artifact.

## How to review or update

From the repository root, with Node.js 22, pnpm 10 and the Rust toolchain installed:

~~~bash
pnpm install --frozen-lockfile
cargo metadata --locked --format-version 1 >/dev/null

python3 scripts/generate-third-party-licenses.py
python3 scripts/generate-third-party-licenses.py --check
~~~

This reproduces third-party-licenses.txt based on the dependency versions actually installed. If lockfiles change, regenerate and review the notice inventory alongside the dependency change.

To inspect **original license wording** and any copyright notices, consult the relevant package's LICENSE, LICENSE.md, NOTICE or other bundled files in node_modules and the Cargo registry package sources, and consult its project reference.

## Project ownership boundary

Zerant source and documentation remain proprietary as stated in [LICENSE](LICENSE). API integration examples carry a limited integration-specific reuse permission described there. None of these permissions supersede third-party licenses.

For a production or commercial redistribution, confirm the complete set of obligations with a qualified reviewer before treating this file as comprehensive.
