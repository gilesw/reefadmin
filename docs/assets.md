# Assets and theme

ReefAdmin embeds Bootstrap 5.3.8, its JavaScript bundle (including Popper 2.11.8), HTMX 2.0.11, and a small admin stylesheet/script. Assets are served under the configured prefix at `/_static/`. Applications can serve these assets without Node or a content delivery network (CDN).

Vendor URLs include versions and use immutable caching. `admin.css` and `admin.js` are revalidated. Unknown asset paths return 404.

`.stylesheet(url)` and `.script(url)` replace the Bootstrap files. `.extra_stylesheet(url)` and `.extra_script(url)` append application assets. CSS loads in this order: Bootstrap, admin CSS, extra stylesheets, the `styles` template block. Scripts load Bootstrap, HTMX, admin JS, extra scripts, then the `scripts` block.

Use Bootstrap CSS variables for colours and typography. `--admin-cell-space` and `--admin-sidebar-space` adjust table and sidebar padding. Keep custom styles in an extra stylesheet; replace templates for structural changes.

## Updating vendor files

Download pinned distributions from these sources into the matching version directories under `static/vendor/`:

- [Bootstrap CSS](https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/dist/css/bootstrap.min.css), [bundle JS](https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/dist/js/bootstrap.bundle.min.js), [MIT licence](https://cdn.jsdelivr.net/npm/bootstrap@5.3.8/LICENSE).
- [Popper MIT licence](https://cdn.jsdelivr.net/npm/@popperjs/core@2.11.8/LICENSE.md) for the copy included in Bootstrap's bundle.
- [HTMX JS](https://cdn.jsdelivr.net/npm/htmx.org@2.0.11/dist/htmx.min.js), [licence](https://cdn.jsdelivr.net/npm/htmx.org@2.0.11/LICENSE).

When updating a bundled dependency:

1. Read its release notes and select a version.
2. Download the distribution and licence files into its version directory. Keep the files and licence notices unchanged. Omit source maps.
3. Update the versioned paths in asset routes, default URLs and tests.
4. Regenerate `static/vendor/SHA256SUMS` for the bundled files.
5. Run `mise run package:check` and `mise run test:browser`.

Applications use the embedded files without a separate download.
