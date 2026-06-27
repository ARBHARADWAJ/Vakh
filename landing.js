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
});
