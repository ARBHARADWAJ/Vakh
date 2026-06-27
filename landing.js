document.addEventListener('DOMContentLoaded', () => {
  // Theme Switching Logic
  const themeToggleBtn = document.getElementById('theme-toggle-btn');
  const sunIcon = themeToggleBtn.querySelector('.sun-icon');
  const moonIcon = themeToggleBtn.querySelector('.moon-icon');

  // Check saved theme or system preference
  const savedTheme = localStorage.getItem('vakh-theme');
  const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

  // Default is white theme. We switch to dark if savedTheme is 'dark' or if no savedTheme but system prefers dark
  if (savedTheme === 'dark' || (!savedTheme && systemPrefersDark)) {
    document.body.classList.add('dark-theme');
    sunIcon.style.display = 'block';
    moonIcon.style.display = 'none';
  } else {
    document.body.classList.remove('dark-theme');
    sunIcon.style.display = 'none';
    moonIcon.style.display = 'block';
  }

  themeToggleBtn.addEventListener('click', () => {
    const isDark = document.body.classList.toggle('dark-theme');
    localStorage.setItem('vakh-theme', isDark ? 'dark' : 'light');
    if (isDark) {
      sunIcon.style.display = 'block';
      moonIcon.style.display = 'none';
    } else {
      sunIcon.style.display = 'none';
      moonIcon.style.display = 'block';
    }
  });

  // Horizontal Features Carousel Controls & Auto-scroll
  const carouselContainer = document.querySelector('.features-carousel-container');
  const prevBtn = document.getElementById('carousel-prev');
  const nextBtn = document.getElementById('carousel-next');

  if (carouselContainer && prevBtn && nextBtn) {
    const scrollAmount = 340; // Card width + gap
    let autoScrollInterval = null;

    const scrollNext = () => {
      const maxScrollLeft = carouselContainer.scrollWidth - carouselContainer.clientWidth;
      if (carouselContainer.scrollLeft >= maxScrollLeft - 10) {
        carouselContainer.scrollTo({ left: 0, behavior: 'smooth' });
      } else {
        carouselContainer.scrollBy({ left: scrollAmount, behavior: 'smooth' });
      }
    };

    const scrollPrev = () => {
      if (carouselContainer.scrollLeft <= 10) {
        const maxScrollLeft = carouselContainer.scrollWidth - carouselContainer.clientWidth;
        carouselContainer.scrollTo({ left: maxScrollLeft, behavior: 'smooth' });
      } else {
        carouselContainer.scrollBy({ left: -scrollAmount, behavior: 'smooth' });
      }
    };

    prevBtn.addEventListener('click', scrollPrev);
    nextBtn.addEventListener('click', scrollNext);

    // Auto-scroll loop
    const startAutoScroll = () => {
      stopAutoScroll();
      autoScrollInterval = setInterval(scrollNext, 3500);
    };

    const stopAutoScroll = () => {
      if (autoScrollInterval) clearInterval(autoScrollInterval);
    };

    startAutoScroll();

    // Pause auto-scroll when user interacts or hovers
    carouselContainer.addEventListener('mouseenter', stopAutoScroll);
    carouselContainer.addEventListener('mouseleave', startAutoScroll);
    carouselContainer.addEventListener('touchstart', stopAutoScroll, { passive: true });
    carouselContainer.addEventListener('touchend', startAutoScroll);
  }

  // Live GitHub Release Download Counter & Outer Water Ripple Celebration
  const countValEl = document.getElementById('download-count-val');
  const waterCard = document.getElementById('water-wave-card');
  const outerRippleContainer = document.getElementById('outer-ripple-container');
  const counterSection = document.getElementById('downloads-counter');
  const downloadSection = document.getElementById('download');

  // Load persisted count from localStorage or default to 500
  const STORAGE_KEY = 'vakh_global_download_count';
  let baseDownloadCount = parseInt(localStorage.getItem(STORAGE_KEY), 10) || 500;

  function updateCountDisplay() {
    if (countValEl) {
      countValEl.textContent = baseDownloadCount.toLocaleString();
    }
  }

  // Initial display update
  updateCountDisplay();

  // Fetch real download count from GitHub API and sync with local storage
  async function fetchGitHubDownloads() {
    try {
      const res = await fetch('https://api.github.com/repos/ARBHARADWAJ/Vakh/releases');
      if (res.ok) {
        const releases = await res.json();
        let total = 0;
        releases.forEach(rel => {
          if (rel.assets) {
            rel.assets.forEach(asset => {
              total += asset.download_count || 0;
            });
          }
        });
        if (total > 0) {
          baseDownloadCount = Math.max(baseDownloadCount, total, 500);
          localStorage.setItem(STORAGE_KEY, baseDownloadCount);
        }
      }
    } catch (e) {
      console.log('Using static/persisted download count fallback');
    }
    updateCountDisplay();
  }

  fetchGitHubDownloads();

  // 3 Concentric Outer Digital Water Waves Radiating Outward Around Card Container
  function triggerOuterWaterCelebration() {
    if (!outerRippleContainer) return;

    // Clear existing waves if re-triggered
    outerRippleContainer.innerHTML = '';

    for (let i = 0; i < 3; i++) {
      const wave = document.createElement('div');
      wave.className = `outer-digital-wave outer-ring-${i + 1}`;
      wave.style.animationDelay = `${i * 0.25}s`;
      outerRippleContainer.appendChild(wave);

      setTimeout(() => {
        wave.remove();
      }, 1800 + (i * 300));
    }
  }

  if (waterCard) {
    waterCard.addEventListener('click', () => {
      triggerOuterWaterCelebration();
    });
  }

  // 1. Navigation & Hero Download buttons -> Smooth scroll to main #download section
  const heroAndNavDownloadBtns = document.querySelectorAll('.hero-actions .btn-primary, .nav-btn-primary');
  heroAndNavDownloadBtns.forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      if (downloadSection) {
        downloadSection.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    });
  });

  // 2. Installer Download Action -> Increment persisted count, auto-scroll to counter, & trigger waves
  const installerDownloadBtns = document.querySelectorAll('.download-btn-main, a[href*="releases/download"]');
  installerDownloadBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      baseDownloadCount++;
      localStorage.setItem(STORAGE_KEY, baseDownloadCount);
      updateCountDisplay();

      // Smooth auto scroll up to global adoption counter section
      if (counterSection) {
        setTimeout(() => {
          counterSection.scrollIntoView({ behavior: 'smooth', block: 'center' });
          triggerOuterWaterCelebration();
        }, 300);
      }
    });
  });
});
