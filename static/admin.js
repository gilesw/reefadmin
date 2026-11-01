// Delegated listeners also cover controls inserted by partial page updates.
(() => {
  document.addEventListener('click', (event) => {
    if (event.target.closest('[data-admin-reload]')) window.location.reload();
  });

  document.addEventListener('change', (event) => {
    if (!event.target.matches('[data-admin-select-all]')) return;
    const form = event.target.closest('form');
    form.querySelectorAll('input[name="selected"]').forEach((box) => {
      box.checked = event.target.checked;
    });
  });

  document.addEventListener('submit', (event) => {
    const form = event.target;
    let message = form.dataset.adminConfirm;
    if (form.matches('[data-admin-actions]')) {
      message = form.querySelector('select[name="action"]')?.selectedOptions[0]?.dataset.confirm;
    }
    if (message && !window.confirm(message)) event.preventDefault();
  });
})();

// Only accept fragments produced by admin handlers, never a redirected login page.
document.addEventListener('htmx:beforeSwap', (event) => {
  const marker = event.detail.xhr.getResponseHeader('X-ReefAdmin-Fragment')
    ?? event.detail.xhr.getResponseHeader('X-Loco-Admin-Fragment');
  if (marker !== 'true') {
    event.detail.shouldSwap = false;
    const error = document.getElementById('admin-request-error');
    if (error) { error.hidden = false; error.focus(); }
  }
});
for (const name of ['htmx:responseError', 'htmx:sendError', 'htmx:timeout']) {
  document.addEventListener(name, () => {
    const error = document.getElementById('admin-request-error');
    if (error) { error.hidden = false; error.focus(); }
  });
}
document.addEventListener('htmx:afterSwap', (event) => {
  const error = document.getElementById('admin-request-error');
  if (error) error.hidden = true;
  const status = document.getElementById('admin-status');
  if (status) status.textContent = event.detail.target.tagName === 'TR' ? 'Row updated. Its selection has been cleared.' : 'Results updated.';
  if (event.detail.target.id === 'admin-results' && document.activeElement === document.body) {
    document.getElementById('admin-results')?.focus({ preventScroll: true });
  }
});

// Keep field names suitable for normal form submission; lookup requests use q.
document.addEventListener('htmx:configRequest', (event) => {
  const input = event.detail.elt;
  if (input.matches('[data-admin-autocomplete]')) event.detail.parameters.q = input.value;
});

// Disabling a focused toggle can blur it before HTMX captures focus for the swap.
// Remember it before the request, but do not steal focus if the user moved elsewhere.
const toggleFocus = new WeakMap();
document.addEventListener('htmx:beforeRequest', (event) => {
  const button = event.detail.elt;
  if (button.matches('button[form="admin-toggle"]') && document.activeElement === button) {
    toggleFocus.set(event.detail.xhr, button.id);
  }
});
document.addEventListener('htmx:afterSettle', (event) => {
  const id = toggleFocus.get(event.detail.xhr);
  if (id && document.activeElement === document.body) {
    document.getElementById(id)?.focus({ preventScroll: true });
  }
  toggleFocus.delete(event.detail.xhr);
});
