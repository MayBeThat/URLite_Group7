// URLite – app.js (plain JS, no build step, API on same origin)

// === STATE ===
let currentTab  = 'login';      // which tab is active in auth modal
let lastResult  = null;         // last { short_code, short_url } from /shorten
let linkHistory = [];           // all user links (loaded from GET /urls)
let clickChart  = null;         // Chart.js instance (destroyed & recreated each stats load)
let pendingUrl  = null;         // URL saved when user tried to shorten before logging in
let prevPage    = 'page-main';  // page before navigating to stats


// === PAGE NAVIGATION ===

const PAGES = ['page-main', 'page-dashboard', 'page-stats'];

function showPage(id) {
  PAGES.forEach(p => {
    document.getElementById(p).style.display = (p === id) ? 'block' : 'none';
  });
  updateHeader();
}

function currentPage() {
  return PAGES.find(p => document.getElementById(p).style.display !== 'none') || 'page-main';
}

function goHome() {
  closeAuthModal();
  showPage('page-main');
}

function showDashboard() {
  showPage('page-dashboard');
  loadUserLinks();
}

function goBack() {
  if (prevPage === 'page-dashboard') {
    showDashboard();
  } else {
    goHome();
  }
}


// === AUTH MODAL ===

function openAuthModal(tab) {
  document.getElementById('modal-backdrop').classList.add('open');
  document.getElementById('modal-container').classList.add('open');
  switchTab(tab);
  setTimeout(() => document.getElementById('auth-username').focus(), 50);
}

function closeAuthModal() {
  document.getElementById('modal-backdrop').classList.remove('open');
  document.getElementById('modal-container').classList.remove('open');
  hide('auth-error');
  hide('auth-success');
}

// Close modal on ESC key
document.addEventListener('keydown', e => {
  if (e.key === 'Escape') closeAuthModal();
});


// === HEADER ===

function updateHeader() {
  const token   = localStorage.getItem('auth_token');
  const authBar = document.getElementById('auth-bar');
  const nav     = document.getElementById('main-nav');

  if (token) {
    const username = getUsernameFromToken(token);
    nav.style.display = 'flex';
    authBar.innerHTML = `
      <span class="header-username">${escapeHtml(username || '')}</span>
      <button class="btn btn-ghost" onclick="logout()">Log out</button>
    `;
    // Highlight active nav item
    const page = currentPage();
    document.querySelectorAll('#main-nav a').forEach(a => {
      const fn = a.getAttribute('onclick') || '';
      const active = (fn.includes('goHome') && page === 'page-main') ||
                     (fn.includes('showDashboard') && page === 'page-dashboard');
      a.classList.toggle('nav-active', active);
    });
  } else {
    nav.style.display = 'none';
    authBar.innerHTML = `
      <button class="btn btn-ghost"  onclick="openAuthModal('login')">Log in</button>
      <button class="btn-nav-signup" onclick="openAuthModal('register')">Sign up</button>
    `;
  }
}


// === AUTHENTICATION ===

function switchTab(tab) {
  currentTab = tab;

  // Update active tab button style
  document.querySelectorAll('.tab').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.tab === tab);
  });

  // Update submit button label
  document.getElementById('auth-submit').textContent =
    tab === 'login' ? 'Log in' : 'Create account';

  // Clear messages
  hide('auth-error');
  hide('auth-success');

  // Update footer link
  const footer = document.getElementById('auth-switch');
  if (tab === 'login') {
    footer.innerHTML = `Don't have an account?
      <button onclick="switchTab('register')">Sign up for free</button>`;
  } else {
    footer.innerHTML = `Already have an account?
      <button onclick="switchTab('login')">Log in</button>`;
  }
}

async function handleAuth(event) {
  event.preventDefault();
  hide('auth-error');
  hide('auth-success');

  const username = document.getElementById('auth-username').value.trim();
  const password = document.getElementById('auth-password').value;
  const btn      = document.getElementById('auth-submit');

  btn.disabled    = true;
  btn.textContent = 'Please wait…';

  try {
    if (currentTab === 'login') {
      // --- Login ---
      const data = await apiRequest('/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      });
      localStorage.setItem('auth_token', data.token);
      goHome();

      // If user pasted a URL before logging in, shorten it now
      if (pendingUrl) {
        document.getElementById('long-url').value = pendingUrl;
        pendingUrl = null;
        document.getElementById('shorten-form').requestSubmit();
      }

    } else {
      // --- Register ---
      const data = await apiRequest('/auth/register', {
        method: 'POST',
        body: JSON.stringify({ username, password }),
      });
      show('auth-success');
      document.getElementById('auth-success').textContent = data.message;
      switchTab('login');
      document.getElementById('auth-username').value = '';
      document.getElementById('auth-password').value = '';
    }

  } catch (err) {
    show('auth-error');
    document.getElementById('auth-error').textContent = err.message;

  } finally {
    btn.disabled    = false;
    btn.textContent = currentTab === 'login' ? 'Log in' : 'Create account';
  }
}

function logout() {
  localStorage.removeItem('auth_token');
  linkHistory = [];
  lastResult  = null;
  pendingUrl  = null;
  hide('result-banner');
  showPage('page-main');
  updateHeader();
}


// === URL SHORTENING ===

async function handleShorten(event) {
  event.preventDefault();
  hide('shorten-error');
  hide('result-banner');

  const urlInput = document.getElementById('long-url');
  const url      = urlInput.value.trim();

  // Must be logged in to shorten
  if (!localStorage.getItem('auth_token')) {
    pendingUrl = url;
    openAuthModal('login');
    return;
  }

  const btn = document.getElementById('shorten-btn');
  btn.disabled    = true;
  btn.textContent = 'Shortening…';

  try {
    const data = await apiRequest('/shorten', {
      method: 'POST',
      body: JSON.stringify({ original_url: url }),
    });

    lastResult = data;

    // Show result banner
    const link = document.getElementById('result-link');
    link.href        = data.short_url;
    link.textContent = data.short_url;
    show('result-banner');

    // Prepend to history (click_count starts at 0 for new links)
    linkHistory.unshift({ ...data, original_url: url, created_at: new Date().toISOString(), click_count: 0 });

    urlInput.value = '';

  } catch (err) {
    show('shorten-error');
    document.getElementById('shorten-error').textContent = err.message;

  } finally {
    btn.disabled    = false;
    btn.textContent = 'Shorten!';
  }
}

function copyLink(btn) {
  if (!lastResult) return;
  navigator.clipboard.writeText(lastResult.short_url);
  btn.textContent = '✓ Copied!';
  setTimeout(() => { btn.textContent = 'Copy link'; }, 2000);
}

async function deleteLink(code) {
  if (!confirm('Xóa link này?')) return;
  try {
    await apiRequest(`/urls/${code}`, { method: 'DELETE' });
    linkHistory = linkHistory.filter(item => item.short_code !== code);
    if (lastResult && lastResult.short_code === code) {
      lastResult = null;
      hide('result-banner');
    }
    renderHistory();
  } catch (err) {
    alert(err.message);
  }
}

async function loadUserLinks() {
  try {
    const items = await apiRequest('/urls');
    linkHistory = items;
    renderHistory();
  } catch (err) {
    // Non-fatal: history just won't show
  }
}

function renderHistory() {
  const list = document.getElementById('history-list');
  if (!list) return;

  // Stat cards
  const linksEl  = document.getElementById('stat-links');
  const clicksEl = document.getElementById('stat-clicks');
  if (linksEl && clicksEl) {
    linksEl.textContent  = linkHistory.length;
    clicksEl.textContent = linkHistory.reduce((s, i) => s + (i.click_count || 0), 0);
  }

  if (linkHistory.length === 0) {
    list.innerHTML = '<tr><td colspan="5" class="empty-row">No links yet.</td></tr>';
    return;
  }

  list.innerHTML = linkHistory.map(item => `
    <tr>
      <td class="link-short">
        <a href="${escapeHtml(item.short_url)}" target="_blank">${escapeHtml(item.short_url)}</a>
      </td>
      <td class="link-original">${escapeHtml(item.original_url)}</td>
      <td class="link-date">${item.created_at.slice(0, 10)}</td>
      <td class="link-clicks">${item.click_count ?? 0}</td>
      <td class="link-action">
        <a href="#" onclick="loadStats('${item.short_code}'); return false;">Stats →</a>
        <button class="btn-delete" onclick="deleteLink('${item.short_code}')">Delete</button>
      </td>
    </tr>
  `).join('');
}


// === STATISTICS ===

async function loadStats(code) {
  prevPage = currentPage();
  showPage('page-stats');

  show('stats-loading');
  hide('stats-error');
  hide('stats-content');

  try {
    const data = await apiRequest(`/stats/${code}`);
    renderStats(data);
    show('stats-content');

  } catch (err) {
    const isNotFound = err.message.toLowerCase().includes('not found');
    document.getElementById('stats-error-title').textContent =
      isNotFound ? '404 – Short code not found' : 'Error loading stats';
    document.getElementById('stats-error-msg').textContent = err.message;
    show('stats-error');

  } finally {
    hide('stats-loading');
  }
}

function renderStats(data) {
  // --- Summary card ---
  document.getElementById('stats-summary').innerHTML = `
    <div class="stats-top">
      <h2 class="stats-code">/${escapeHtml(data.short_code)}</h2>
      <span class="stats-badge">created ${data.created_at.slice(0, 10)}</span>
    </div>
    <a href="${escapeHtml(data.original_url)}" target="_blank" class="stats-original-url">
      ${escapeHtml(data.original_url)}
    </a>
    <hr class="stats-divider" />
    <div>
      <p class="stats-count">${data.click_count}</p>
      <p class="stats-count-label">Total clicks</p>
    </div>
  `;

  // No clicks? hide detail cards and stop here.
  if (data.click_count === 0) {
    hide('stats-chart-card');
    hide('stats-ua-card');
    hide('stats-clicks-card');
    return;
  }

  show('stats-chart-card');
  show('stats-ua-card');
  show('stats-clicks-card');

  // --- Bar chart ---
  renderChart(data.clicks);

  // --- Top User-Agents ---
  const uaCounts = {};
  data.clicks.forEach(c => {
    const ua = c.user_agent || 'Unknown';
    uaCounts[ua] = (uaCounts[ua] || 0) + 1;
  });
  const topUA = Object.entries(uaCounts)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  document.getElementById('stats-ua-body').innerHTML =
    topUA.map(([ua, count]) => `
      <tr>
        <td class="truncate">${escapeHtml(ua)}</td>
        <td class="count-cell">${count}</td>
      </tr>
    `).join('');

  // --- Recent clicks table ---
  document.getElementById('stats-clicks-body').innerHTML =
    data.clicks.slice(0, 20).map(c => `
      <tr>
        <td class="text-muted">${c.clicked_at.replace('T', ' ').slice(0, 19)}</td>
        <td>${escapeHtml(c.ip_address || '—')}</td>
        <td class="truncate text-muted">${escapeHtml(c.user_agent || '—')}</td>
      </tr>
    `).join('');
}

function renderChart(clicks) {
  // Group clicks by day (YYYY-MM-DD → count)
  const byDay = {};
  clicks.forEach(c => {
    const day = c.clicked_at.slice(0, 10);
    byDay[day] = (byDay[day] || 0) + 1;
  });

  const sortedDays = Object.keys(byDay).sort();
  const labels = sortedDays.map(d => d.slice(5));   // MM-DD
  const values = sortedDays.map(d => byDay[d]);

  // Destroy previous chart instance to avoid "canvas already in use" error
  if (clickChart) clickChart.destroy();

  const ctx = document.getElementById('stats-chart').getContext('2d');
  clickChart = new Chart(ctx, {
    type: 'bar',
    data: {
      labels,
      datasets: [{
        label: 'Clicks',
        data: values,
        backgroundColor: '#2563eb',
        borderRadius: 4,
      }],
    },
    options: {
      responsive: true,
      plugins: { legend: { display: false } },
      scales: {
        x: { grid: { display: false } },
        y: { ticks: { precision: 0 }, grid: { color: '#e2e8f0' } },
      },
    },
  });
}


// === UTILITIES ===

// Decode the JWT payload to get the username (stored in 'sub' claim).
// We only read the payload — we never verify the signature in the browser.
function getUsernameFromToken(token) {
  try {
    const payload = JSON.parse(atob(token.split('.')[1]));
    return payload.sub || null;
  } catch {
    return null;
  }
}

// Central fetch wrapper.
// Automatically adds Content-Type and Authorization headers.
// Throws an Error with the server's error message if the response is not OK.
async function apiRequest(path, options = {}) {
  const token = localStorage.getItem('auth_token');

  const headers = {
    'Content-Type': 'application/json',
    ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
    ...(options.headers || {}),
  };

  const response = await fetch(path, { ...options, headers });
  const body     = await response.json();

  if (!response.ok) {
    throw new Error(body.error || `HTTP ${response.status}`);
  }

  return body;
}

// Escape HTML to prevent XSS when injecting user-supplied content into innerHTML.
function escapeHtml(str) {
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Helper shortcuts for display toggling
function show(id) { document.getElementById(id).style.display = 'block'; }
function hide(id) { document.getElementById(id).style.display = 'none';  }


// === STARTUP ===

function init() {
  showPage('page-main');
  if (!localStorage.getItem('auth_token')) {
    switchTab('login');
  }
  updateHeader();
}

init();

document.addEventListener('visibilitychange', () => {
  if (!document.hidden && currentPage() === 'page-dashboard') {
    loadUserLinks();
  }
});
