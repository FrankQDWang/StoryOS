# Approved local supervision interaction contract

US40 uses the author-approved Grafana App v0.3.3. This contract fixes the
implementation direction. It does not reopen visual design. The reference is
local-only under `target/observation/grafana-human-prototype`; runtime builds
must use tracked source only.

## Reference identity

| Relative reference path | SHA-256 |
| --- | --- |
| `plugin/module.js` | `4eb87bf47bef28db43280a621553d8021e49878df3d831adddfa426d0b53925a` |
| `plugin/style.css` | `a0d1966e18f3895e68fe2e011e08fa7380b501dfe0c1ff7f2f67af2753ed7843` |
| `qa/flat/01-overview.png` | `a9fa71cf092a8c87b35e61eea205019e32e83d5c5eb4c788be6805150ac51ca7` |
| `qa/flat/02-history.png` | `9de924346447b135c9ff23216349a29e48ba0aa2630aceb0504e5c4b7c87a2b0` |
| `qa/drawer-spacing/01-summary.png` | `6c470ce5d16e3c9022c0407ce412d5d908d2232cb2cf7d7f5371dabe911d3312` |

## Layout and interaction

- Keep the native Grafana shell. The App starts below its 40 px header.
- Use a fixed 204 px left navigation, light gray navigation background,
  white content, system sans-serif type, and restrained purple selection.
- Use a 72 px content header, 38 px horizontal content padding, 14 px body
  type, 18 px page titles, 15 px section titles, and compact unboxed indicators.
- Keep overview, history, and monitoring health in the left navigation.
  Overview has current runs, stale unfinished records, and recent runs.
- Use flat aligned rows and spacing. Keep input boundaries and visible keyboard
  focus. Do not add nested cards, repeated row rules, or decorative shadows.
- Preserve Chinese product labels from the approved reference. Retained source
  reasons stay in their original language. Grafana owns its logo and system icons;
  App-owned pages need no raster assets or custom icon set.
- History search covers retained history through paginated API reads. Source-page
  filters, order, displayed membership, and scroll survive same-tab reload.
- Poll known row facts in place. New membership, status groups, and order require
  explicit user application. Failed reads label retained data and the last success;
  a successful read clears that error. Never discard reading state on a poll.
- Keep `page`, `run`, `level`, and `file` in the hash route. The next drawer ticket
  owns summary, files, file evidence, and diagnostics. Close restores row focus.
- The drawer fills the App height and touches the right edge. Summary has only
  the Chinese Close action; nested views have contextual Back. Escape closes it;
  browser Back follows visited levels. Each level retains filters and scroll.
- Missing evidence remains unknown. Selection is not execution. A grouped pass
  never establishes file success or duration. Conformity does not prove minimality.

US40-C owns navigation and lists. US40-D owns the evidence drawer. US40 integration
owns the health page and clean rebuild acceptance. US41 owns further anomaly,
cost, graph, timeline, and comparison presentation. Existing dashboards and alerts
remain available. All screenshots and build output stay under `target/observation`.
