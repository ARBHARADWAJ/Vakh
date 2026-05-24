const { invoke } = window.__TAURI__.core;

let appEl;
let statusTextEl;
let toggleBtn;
let playIcon;
let stopIcon;
let waveformBars = [];

window.addEventListener("DOMContentLoaded", () => {
  appEl = document.querySelector("#app");
  statusTextEl = document.querySelector("#status-text");
  toggleBtn = document.querySelector("#toggle-btn");
  playIcon = document.querySelector("#play-icon");
  stopIcon = document.querySelector("#stop-icon");

  // Get all waveform bars
  waveformBars = document.querySelectorAll(".waveform .bar");

  // Handle dragging the window from anywhere on the capsule except buttons
  appEl.addEventListener("mousedown", (e) => {
    if (e.target.tagName !== "BUTTON" && !e.target.closest("button") && !e.target.closest(".icon-btn")) {
      invoke("start_dragging");
    }
  });

  toggleBtn.addEventListener("click", () => toggleListening());

  const dashboardBtn = document.querySelector("#dashboard-btn");
  if (dashboardBtn) {
    dashboardBtn.addEventListener("click", () => {
      invoke("open_dashboard");
    });
  }

  // Load config & apply theme/opacity on startup
  async function applyConfig() {
    try {
      const config = await invoke("get_config");
      updateThemeAndOpacity(config);
    } catch (err) {
      console.error("Failed to load config:", err);
    }
  }
  applyConfig();

  // Listen for config changes from backend
  window.__TAURI__.event.listen("vakh-config-changed", (event) => {
    updateThemeAndOpacity(event.payload);
  });

  function updateThemeAndOpacity(config) {
    if (!config) return;
    if (appEl) {
      appEl.style.background = `rgba(10, 10, 12, ${config.capsule_opacity})`;
      
      // Clear theme classes and apply selected theme
      appEl.className = appEl.className.split(" ").filter(c => !c.startsWith("theme-")).join(" ");
      appEl.classList.add(`theme-${config.theme}`);
    }
  }

  document.querySelector("#close-btn").addEventListener("click", () => {
    hideWindow();
  });

  // ESC to hide logic
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hideWindow();
    }
  });

  // Listen for show event to reset styles
  window.__TAURI__.event.listen("vakh-show", () => {
    appEl.style.transform = "";
    appEl.style.opacity = "";
  });

  // Listen for hide event
  window.__TAURI__.event.listen("vakh-hide", () => {
    hideWindow();
  });

  // Listen for real-time corrections from Thread 2
  window.__TAURI__.event.listen("vakh-correction", (event) => {
    // This is handled by the backend's injector directly in the future,
    // but we can log it or show a small indicator in UI if needed.
    console.log("[Correction]", event.payload);
  });

  // Listen for backend status updates (silence warnings, etc.)
  window.__TAURI__.event.listen("vakh-status", (event) => {
    const data = event.payload;
    if (data.status === "warning") {
      appEl.classList.add("is-warning");
      appEl.classList.remove("is-speaking");
      statusTextEl.innerText = `SILENCE (${data.duration}s)...`;
    } else if (data.status === "active") {
      appEl.classList.remove("is-warning");
      appEl.classList.remove("is-finalizing");
      appEl.classList.remove("is-processing");
      appEl.classList.remove("is-listening");
      appEl.classList.add("is-speaking");
      statusTextEl.innerText = "SPEAKING";
    } else if (data.status === "listening") {
      appEl.classList.remove("is-warning");
      appEl.classList.remove("is-finalizing");
      appEl.classList.remove("is-processing");
      appEl.classList.remove("is-speaking");
      appEl.classList.add("is-listening");
      statusTextEl.innerText = "LISTENING";
    } else if (data.status === "finalizing") {
      appEl.classList.remove("is-warning");
      appEl.classList.remove("is-listening");
      appEl.classList.remove("is-speaking");
      appEl.classList.add("is-finalizing");
      statusTextEl.innerText = "FINALIZING...";
    } else if (data.status === "processing") {
      appEl.classList.remove("is-warning");
      appEl.classList.remove("is-listening");
      appEl.classList.remove("is-speaking");
      appEl.classList.add("is-processing");
      statusTextEl.innerText = "PROCESSING...";
    } else if (data.status === "idle") {
      updateUI("Idle");
      // Reset bars to default
      waveformBars.forEach(bar => {
        bar.style.height = "6px";
        bar.style.background = "#636366";
      });
    } else if (data.status === "level") {
      // Audio level for visualization - animate bars based on level
      if (appEl.classList.contains("is-listening") || appEl.classList.contains("is-speaking")) {
        updateWaveform(data.level);
      }
    }
  });

  // Update waveform bars based on audio level
  function updateWaveform(level) {
    const baseHeights = [8, 14, 10, 16, 6];
    const maxHeights = [16, 22, 18, 24, 14];

    waveformBars.forEach((bar, index) => {
      const minHeight = baseHeights[index];
      const maxHeight = maxHeights[index];
      const randomOffset = Math.random() * 0.3 + 0.85;
      const height = minHeight + (maxHeight - minHeight) * level * randomOffset;
      bar.style.height = `${height}px`;
      
      // Clear inline styles to let CSS handle the colors (Green/Blue)
      bar.style.background = "";
      bar.style.boxShadow = "";
    });
  }
});

function hideWindow() {
  appEl.style.transform = "scale(0.9)";
  appEl.style.opacity = "0";
  setTimeout(() => {
    invoke("hide_window");
  }, 400);
}

async function toggleListening() {
  try {
    const newState = await invoke("toggle_listening");
    updateUI(newState);
  } catch (error) {
    console.error("Failed to toggle listening:", error);
  }
}

function updateUI(state) {
  if (state === "Listening" || state === "Processing") {
    appEl.className = "is-listening";
    statusTextEl.innerText = "LISTENING";
    playIcon.style.display = "none";
    stopIcon.style.display = "block";
    stopIcon.style.color = "#ff453a";
  } else {
    appEl.className = "is-paused";
    statusTextEl.innerText = "PAUSED";
    playIcon.style.display = "block";
    playIcon.style.color = "#32d74b";
    stopIcon.style.display = "none";
  }
}
