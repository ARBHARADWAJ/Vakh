const { invoke } = window.__TAURI__.core;

// Global State
let currentConfig = null;
let historyLogs = [];

window.addEventListener("DOMContentLoaded", () => {
  initWindowControls();
  initTabNavigation();
  initOverviewTab();
  initHistoryTab();
  initSettingsTab();
  initThemesTab();
  
  // Listen for config changes from backend (e.g. from main capsule or dashboard saving)
  window.__TAURI__.event.listen("vakh-config-changed", (event) => {
    if (event.payload) {
      currentConfig = event.payload;
      syncConfigToUI(currentConfig);
    }
  });

  // Initial load
  loadConfigAndSync();
});

// ------------------------------------------------------------
// Window Title Bar Controls
// ------------------------------------------------------------
function initWindowControls() {
  document.querySelector("#close-btn").addEventListener("click", () => {
    invoke("hide_dashboard");
  });

  document.querySelector("#min-btn").addEventListener("click", () => {
    invoke("minimize_dashboard");
  });
}

// ------------------------------------------------------------
// Tab Switching System
// ------------------------------------------------------------
function initTabNavigation() {
  const links = document.querySelectorAll(".nav-link");
  const panes = document.querySelectorAll(".tab-pane");

  links.forEach(link => {
    link.addEventListener("click", (e) => {
      e.preventDefault();
      const tabId = link.getAttribute("data-tab");

      links.forEach(l => l.classList.remove("active"));
      panes.forEach(p => p.classList.remove("active"));

      link.classList.add("active");
      document.querySelector(`#tab-${tabId}`).classList.add("active");

      // Trigger loads
      if (tabId === "overview") {
        loadStats();
      } else if (tabId === "history") {
        loadHistoryLogs();
      } else if (tabId === "settings" || tabId === "themes") {
        loadConfigAndSync();
      }
    });
  });
}

// ------------------------------------------------------------
// 1. Overview Tab (Stats & Progress List)
// ------------------------------------------------------------
function initOverviewTab() {
  loadStats();
}

async function loadStats() {
  try {
    const stats = await invoke("get_stats");
    document.querySelector("#stat-sessions").innerText = stats.total_sessions;
    document.querySelector("#stat-words").innerText = stats.total_words;

    const listContainer = document.querySelector("#top-apps-list");
    listContainer.innerHTML = "";

    if (stats.top_apps && stats.top_apps.length > 0) {
      // Find max count to calculate percentage
      const maxCount = Math.max(...stats.top_apps.map(item => item[1]));

      stats.top_apps.forEach(([appName, count]) => {
        const percentage = maxCount > 0 ? (count / maxCount) * 100 : 0;
        
        const item = document.createElement("div");
        item.className = "app-progress-item";
        item.innerHTML = `
          <div class="app-progress-header">
            <span class="app-name">${appName}</span>
            <span class="app-count">${count} sessions</span>
          </div>
          <div class="progress-track">
            <div class="progress-fill" style="width: ${percentage}%"></div>
          </div>
        `;
        listContainer.appendChild(item);
      });
    } else {
      listContainer.innerHTML = `
        <div class="empty-state">
          No target apps captured yet. Dictate into any window to see stats here!
        </div>
      `;
    }
  } catch (err) {
    console.error("Failed to load stats:", err);
  }
}

// ------------------------------------------------------------
// 2. History Tab (SQLite Log Browsing)
// ------------------------------------------------------------
function initHistoryTab() {
  const searchInput = document.querySelector("#history-search");
  const clearBtn = document.querySelector("#clear-all-btn");

  searchInput.addEventListener("input", (e) => {
    filterLogs(e.target.value);
  });

  clearBtn.addEventListener("click", async () => {
    const confirmClear = confirm("Are you sure you want to permanently clear all dictation history logs?");
    if (confirmClear) {
      try {
        await invoke("clear_logs");
        loadHistoryLogs();
        loadStats();
      } catch (err) {
        console.error("Failed to clear logs:", err);
      }
    }
  });
}

async function loadHistoryLogs() {
  try {
    historyLogs = await invoke("get_dictation_logs");
    renderHistoryLogs(historyLogs);
  } catch (err) {
    console.error("Failed to load logs:", err);
  }
}

function renderHistoryLogs(logs) {
  const tbody = document.querySelector("#history-tbody");
  const emptyState = document.querySelector("#history-empty-state");
  tbody.innerHTML = "";

  if (logs.length === 0) {
    emptyState.style.display = "block";
    return;
  }
  
  emptyState.style.display = "none";

  logs.forEach(log => {
    const row = document.createElement("tr");
    row.id = `log-row-${log.id}`;
    
    const appName = log.target_app || "Unknown App";
    
    row.innerHTML = `
      <td><span class="log-date">${log.timestamp}</span></td>
      <td><span class="app-badge">${appName}</span></td>
      <td><div class="log-text">${escapeHtml(log.transcribed_text)}</div></td>
      <td>
        <div class="row-actions">
          <button class="icon-action-btn copy" title="Copy to Clipboard">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"></path>
            </svg>
          </button>
          <button class="icon-action-btn delete" title="Delete entry">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"></path>
            </svg>
          </button>
        </div>
      </td>
    `;

    // Copy action
    row.querySelector(".copy").addEventListener("click", () => {
      navigator.clipboard.writeText(log.transcribed_text);
      showTemporaryToast("Copied to clipboard!");
    });

    // Delete action
    row.querySelector(".delete").addEventListener("click", async () => {
      try {
        await invoke("delete_log", { id: log.id });
        row.remove();
        historyLogs = historyLogs.filter(l => l.id !== log.id);
        if (historyLogs.length === 0) {
          emptyState.style.display = "block";
        }
        loadStats();
      } catch (err) {
        console.error("Failed to delete log:", err);
      }
    });

    tbody.appendChild(row);
  });
}

function filterLogs(query) {
  const lowerQuery = query.toLowerCase().trim();
  if (lowerQuery === "") {
    renderHistoryLogs(historyLogs);
    return;
  }

  const filtered = historyLogs.filter(log => {
    return (
      log.transcribed_text.toLowerCase().includes(lowerQuery) ||
      (log.target_app && log.target_app.toLowerCase().includes(lowerQuery)) ||
      log.timestamp.toLowerCase().includes(lowerQuery)
    );
  });

  renderHistoryLogs(filtered);
}

// ------------------------------------------------------------
// 3. Settings Tab (VAD & Key Injection Configurations)
// ------------------------------------------------------------
async function syncModelStatus() {
  try {
    const status = await invoke("get_model_status");
    const pathDisplay = document.querySelector("#model-path-display");
    const statusHint = document.querySelector("#model-status-hint");
    const resetContainer = document.querySelector("#reset-model-container");

    if (status.is_custom) {
      pathDisplay.value = status.path;
      statusHint.innerText = `Using custom model (${status.size_mb} MB). Restart VAKH to load.`;
      resetContainer.style.display = "block";
    } else {
      pathDisplay.value = status.path;
      statusHint.innerText = `Active model. You can upload a custom GGML Whisper model (.bin) to replace the default tiny.en.`;
      resetContainer.style.display = "none";
    }
  } catch (err) {
    console.error("Failed to sync model status:", err);
  }
}

function initSettingsTab() {
  const form = document.querySelector("#settings-form");
  const silenceSlider = document.querySelector("#setting-silence");
  const typingSlider = document.querySelector("#setting-typing");
  const backspaceSlider = document.querySelector("#setting-backspace");

  // Output feedback binding
  silenceSlider.addEventListener("input", (e) => {
    document.querySelector("#val-silence").innerText = parseFloat(e.target.value).toFixed(1);
  });
  typingSlider.addEventListener("input", (e) => {
    document.querySelector("#val-typing").innerText = e.target.value;
  });
  backspaceSlider.addEventListener("input", (e) => {
    document.querySelector("#val-backspace").innerText = e.target.value;
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (!currentConfig) return;

    const updatedConfig = {
      ...currentConfig,
      silence_timeout: parseFloat(silenceSlider.value),
      vad_sensitivity: parseInt(document.querySelector("#setting-vad").value),
      language: document.querySelector("#setting-language").value,
      typing_delay: parseInt(typingSlider.value),
      backspace_delay: parseInt(backspaceSlider.value),
      threads: parseInt(document.querySelector("#setting-threads").value)
    };

    const statusEl = document.querySelector("#settings-status");
    try {
      await invoke("save_config", { config: updatedConfig });
      currentConfig = updatedConfig;
      
      statusEl.innerText = "Settings Saved Successfully!";
      statusEl.className = "status-msg success";
      
      setTimeout(() => {
        statusEl.className = "status-msg";
      }, 3000);
    } catch (err) {
      statusEl.innerText = "Failed to save settings: " + err;
      statusEl.className = "status-msg error";
    }
  });

  // Whisper Model Updates Event Listeners
  const changeModelBtn = document.querySelector("#change-model-btn");
  const resetModelBtn = document.querySelector("#reset-model-btn");
  const statusEl = document.querySelector("#settings-status");

  changeModelBtn.addEventListener("click", async () => {
    try {
      const msg = await invoke("change_model");
      statusEl.innerText = msg;
      statusEl.className = "status-msg success";
      await syncModelStatus();
      setTimeout(() => {
        statusEl.className = "status-msg";
      }, 5000);
    } catch (err) {
      if (err !== "cancelled") {
        statusEl.innerText = "Failed to change model: " + err;
        statusEl.className = "status-msg error";
        setTimeout(() => {
          statusEl.className = "status-msg";
        }, 5000);
      }
    }
  });

  resetModelBtn.addEventListener("click", async () => {
    try {
      const msg = await invoke("reset_model");
      statusEl.innerText = msg;
      statusEl.className = "status-msg success";
      await syncModelStatus();
      setTimeout(() => {
        statusEl.className = "status-msg";
      }, 5000);
    } catch (err) {
      statusEl.innerText = "Failed to reset model: " + err;
      statusEl.className = "status-msg error";
      setTimeout(() => {
        statusEl.className = "status-msg";
      }, 5000);
    }
  });

  // Load initial model status
  syncModelStatus();

  // Application Update Event Listeners
  const checkUpdatesBtn = document.querySelector("#check-updates-btn");
  const updateContainer = document.querySelector("#update-download-container");
  const updateTitle = document.querySelector("#update-version-title");
  const updateChangelog = document.querySelector("#update-changelog-text");
  const installUpdateBtn = document.querySelector("#install-update-btn");
  let activeDownloadUrl = null;

  checkUpdatesBtn.addEventListener("click", async () => {
    checkUpdatesBtn.innerText = "Checking...";
    checkUpdatesBtn.disabled = true;
    try {
      const res = await invoke("check_for_updates");
      if (res.has_update) {
        updateTitle.innerText = `New Version Available: ${res.latest_version} (Current: ${res.current_version})`;
        updateChangelog.innerText = res.changelog;
        activeDownloadUrl = res.download_url;
        updateContainer.style.display = "block";
        
        statusEl.innerText = "Updates found!";
        statusEl.className = "status-msg success";
      } else {
        updateContainer.style.display = "none";
        statusEl.innerText = "VAKH is up to date!";
        statusEl.className = "status-msg success";
      }
      setTimeout(() => {
        statusEl.className = "status-msg";
      }, 4000);
    } catch (err) {
      statusEl.innerText = "Check update failed: " + err;
      statusEl.className = "status-msg error";
      setTimeout(() => {
        statusEl.className = "status-msg";
      }, 5000);
    } finally {
      checkUpdatesBtn.innerText = "Check for Updates";
      checkUpdatesBtn.disabled = false;
    }
  });

  installUpdateBtn.addEventListener("click", async () => {
    if (!activeDownloadUrl) return;
    installUpdateBtn.innerText = "Downloading & Installing...";
    installUpdateBtn.disabled = true;
    try {
      statusEl.innerText = "Downloading update... The app will close and restart automatically.";
      statusEl.className = "status-msg success";
      await invoke("download_and_install_update", { url: activeDownloadUrl });
    } catch (err) {
      statusEl.innerText = "Installation failed: " + err;
      statusEl.className = "status-msg error";
      installUpdateBtn.innerText = "Download & Install Now";
      installUpdateBtn.disabled = false;
    }
  });
}

// ------------------------------------------------------------
// 4. Themes Tab (Accent picker & Transparency)
// ------------------------------------------------------------
function initThemesTab() {
  const opacitySlider = document.querySelector("#setting-opacity");
  const dashboardOpacitySlider = document.querySelector("#setting-dashboard-opacity");
  const themeCards = document.querySelectorAll(".theme-card");

  opacitySlider.addEventListener("input", (e) => {
    const val = parseFloat(e.target.value);
    document.querySelector("#val-opacity").innerText = val.toFixed(2);
    
    // Live update preview capsule backdrop opacity
    const preview = document.querySelector("#capsule-preview");
    if (preview) {
      preview.style.background = `rgba(10, 10, 12, ${val})`;
    }

    // Save configuration change immediately
    saveThemeOrOpacity(val, null, null);
  });

  if (dashboardOpacitySlider) {
    dashboardOpacitySlider.addEventListener("input", (e) => {
      const val = parseFloat(e.target.value);
      document.querySelector("#val-dashboard-opacity").innerText = val.toFixed(2);
      
      // Live update dashboard container background
      const dashboardApp = document.querySelector("#dashboard-app");
      if (dashboardApp) {
        dashboardApp.style.background = `rgba(10, 10, 14, ${val})`;
      }

      // Save configuration change immediately
      saveThemeOrOpacity(null, null, val);
    });
  }

  themeCards.forEach(card => {
    card.addEventListener("click", () => {
      const selectedTheme = card.getAttribute("data-theme");
      
      themeCards.forEach(c => c.classList.remove("active"));
      card.classList.add("active");

      // Update local dashboard theme wrapper class to preview glow instantly
      document.body.className = `theme-${selectedTheme}`;
      
      // Save configuration change immediately
      saveThemeOrOpacity(null, selectedTheme, null);
    });
  });
}

async function saveThemeOrOpacity(opacity, theme, dashboardOpacity) {
  if (!currentConfig) return;
  
  const updatedConfig = {
    ...currentConfig,
    capsule_opacity: opacity !== null ? opacity : currentConfig.capsule_opacity,
    theme: theme !== null ? theme : currentConfig.theme,
    dashboard_opacity: dashboardOpacity !== null && dashboardOpacity !== undefined ? dashboardOpacity : currentConfig.dashboard_opacity
  };

  try {
    await invoke("save_config", { config: updatedConfig });
    currentConfig = updatedConfig;
  } catch (err) {
    console.error("Failed to update theme/opacity configuration:", err);
  }
}

// ------------------------------------------------------------
// Configuration Loader & Sync Controls
// ------------------------------------------------------------
async function loadConfigAndSync() {
  try {
    currentConfig = await invoke("get_config");
    syncConfigToUI(currentConfig);
  } catch (err) {
    console.error("Failed to load config:", err);
  }
}

function syncConfigToUI(config) {
  if (!config) return;

  // 1. Audio and VAD
  const silenceSlider = document.querySelector("#setting-silence");
  silenceSlider.value = config.silence_timeout;
  document.querySelector("#val-silence").innerText = parseFloat(config.silence_timeout).toFixed(1);
  
  document.querySelector("#setting-vad").value = config.vad_sensitivity;
  document.querySelector("#setting-language").value = config.language;

  // 2. Typing delays & performance
  const typingSlider = document.querySelector("#setting-typing");
  typingSlider.value = config.typing_delay;
  document.querySelector("#val-typing").innerText = config.typing_delay;

  const backspaceSlider = document.querySelector("#setting-backspace");
  backspaceSlider.value = config.backspace_delay;
  document.querySelector("#val-backspace").innerText = config.backspace_delay;

  document.querySelector("#setting-threads").value = config.threads;

  // 3. Opacity Slider & Capsule Preview Opacity
  const opacitySlider = document.querySelector("#setting-opacity");
  opacitySlider.value = config.capsule_opacity;
  document.querySelector("#val-opacity").innerText = parseFloat(config.capsule_opacity).toFixed(2);
  
  const preview = document.querySelector("#capsule-preview");
  if (preview) {
    preview.style.background = `rgba(10, 10, 12, ${config.capsule_opacity})`;
  }

  // Dashboard Opacity Sync
  const dashboardOpacitySlider = document.querySelector("#setting-dashboard-opacity");
  if (dashboardOpacitySlider && config.dashboard_opacity !== undefined) {
    dashboardOpacitySlider.value = config.dashboard_opacity;
    document.querySelector("#val-dashboard-opacity").innerText = parseFloat(config.dashboard_opacity).toFixed(2);
  }
  const dashboardApp = document.querySelector("#dashboard-app");
  if (dashboardApp && config.dashboard_opacity !== undefined) {
    dashboardApp.style.background = `rgba(10, 10, 14, ${config.dashboard_opacity})`;
  }

  // 4. Accent Glow active indicator
  const themeCards = document.querySelectorAll(".theme-card");
  themeCards.forEach(card => {
    if (card.getAttribute("data-theme") === config.theme) {
      card.classList.add("active");
    } else {
      card.classList.remove("active");
    }
  });

  // Apply selected theme to dashboard body container
  document.body.className = `theme-${config.theme}`;
}

// ------------------------------------------------------------
// Utility Helpers
// ------------------------------------------------------------
function escapeHtml(text) {
  if (!text) return "";
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function showTemporaryToast(message) {
  // Check if toast already exists
  let toast = document.querySelector("#toast-alert");
  if (!toast) {
    toast = document.createElement("div");
    toast.id = "toast-alert";
    toast.style.position = "fixed";
    toast.style.bottom = "20px";
    toast.style.right = "20px";
    toast.style.padding = "10px 18px";
    toast.style.background = "rgba(10, 132, 255, 0.9)";
    toast.style.color = "#000";
    toast.style.fontWeight = "700";
    toast.style.borderRadius = "8px";
    toast.style.fontSize = "0.8rem";
    toast.style.boxShadow = "0 4px 12px rgba(10, 132, 255, 0.4)";
    toast.style.transition = "all 0.3s cubic-bezier(0.16, 1, 0.3, 1)";
    toast.style.transform = "translateY(50px)";
    toast.style.opacity = "0";
    toast.style.zIndex = "1000";
    document.body.appendChild(toast);
  }

  toast.innerText = message;
  toast.style.transform = "translateY(0)";
  toast.style.opacity = "1";

  setTimeout(() => {
    toast.style.transform = "translateY(50px)";
    toast.style.opacity = "0";
  }, 2500);
}
