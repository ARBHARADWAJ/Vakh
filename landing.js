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

  // Live GitHub Release Download Counter & Water Ripple Animation
  const countValEl = document.getElementById('download-count-val');
  const waterCard = document.getElementById('water-wave-card');
  const rippleContainer = document.getElementById('ripple-container');
  const downloadBtns = document.querySelectorAll('a[href*="releases/download"], .btn-primary, .nav-btn-primary');

  let baseDownloadCount = 1248; // Base fallback global count

  // Fetch real download count from GitHub API
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
          baseDownloadCount = Math.max(baseDownloadCount, total);
        }
      }
    } catch (e) {
      console.log('Using static download count fallback');
    }
    updateCountDisplay();
  }

  function updateCountDisplay() {
    if (countValEl) {
      countValEl.textContent = baseDownloadCount.toLocaleString();
    }
  }

  fetchGitHubDownloads();

  // Interactive 3 Circular Blue Digital Water Waves
  function createDigitalWaterRipples(e, targetContainer) {
    const rect = targetContainer.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // Create 3 concentric expanding rings
    for (let i = 0; i < 3; i++) {
      const ring = document.createElement('span');
      ring.className = `digital-water-wave wave-ring-${i + 1}`;
      ring.style.left = `${x}px`;
      ring.style.top = `${y}px`;
      ring.style.animationDelay = `${i * 0.18}s`;
      targetContainer.appendChild(ring);

      setTimeout(() => {
        ring.remove();
      }, 1400 + (i * 200));
    }
  }

  if (waterCard && rippleContainer) {
    waterCard.addEventListener('click', (e) => {
      createDigitalWaterRipples(e, rippleContainer);
    });
  }

  // Increment count on download click and trigger digital water ripples
  downloadBtns.forEach(btn => {
    btn.addEventListener('click', (e) => {
      baseDownloadCount++;
      updateCountDisplay();
      if (waterCard && rippleContainer) {
        const rect = waterCard.getBoundingClientRect();
        const fakeEvent = {
          clientX: rect.left + rect.width / 2,
          clientY: rect.top + rect.height / 2
        };
        createDigitalWaterRipples(fakeEvent, rippleContainer);
      }
    });
  });
});
