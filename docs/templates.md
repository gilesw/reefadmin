# Templates

Loco Admin uses Tera 2. Templates are embedded, and `AdminApp::template(name, source)` replaces a named template or adds one. The engine resolves all overrides before it validates them. You can therefore register child and parent templates in either order. If you override the same name more than once, the engine uses the last override.

Replacement layouts must retain the `title`, `content` and `notices` blocks used by built-in child templates. The default layout also provides `styles` and `scripts` blocks. Lists use `list_results.html`, `pagination.html`, `row.html` and `notices.html`; relationship options use `relationship_options.html`.

Use `configure_templates(|tera| { ... })` to register filters, functions or tests before loading templates. This configures Loco Admin's engine; it does not automatically import the host application's templates or filters. Loco Admin does not depend on Loco.

## Moving from MiniJinja

Custom overrides must use Tera 2 syntax. The built-in template names are unchanged, including `macros.html`, which now defines components:

- `{{ m.row_cells(row) }}` becomes `{{ <admin_row row={row} /> }}`.
- `{{ cell_content(cell) }}` becomes `{{ <admin_cell cell={cell} /> }}`.
- Remove macro imports; component names are global to the engine.
- Use explicit `if` branches for fallback text instead of `value or fallback`.
- Use `is defined` when an optional context key may be absent, and `== none` for null values.

HTML templates escape interpolated values by default. URLs retain slashes and escape ampersands; quotes and HTML delimiters are escaped. Use `safe` only for application-owned trusted HTML, never untrusted field values. Escaping HTML does not make arbitrary values safe inside JavaScript or CSS.

The `AdminError::Template` variant now wraps `tera::Error`. If your application matches template errors, update those matches to use `tera::Error`.
