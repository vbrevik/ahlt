        window.toggleTheme = function(newTheme) {
            const html = document.documentElement;

            if (newTheme === 'auto') {
                html.classList.remove('dark');
                localStorage.setItem('theme-preference', 'auto');
                const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
                if (prefersDark) {
                    html.classList.add('dark');
                }
            } else if (newTheme === 'dark') {
                html.classList.add('dark');
                localStorage.setItem('theme-preference', 'dark');
            } else {
                html.classList.remove('dark');
                localStorage.setItem('theme-preference', 'light');
            }
        };

        // Update toggle icon to reflect current theme
        const themeToggle = document.getElementById('theme-toggle');
        function updateThemeIcon() {
            const currentTheme = localStorage.getItem('theme-preference') || 'auto';
            const icon = themeToggle?.querySelector('.theme-icon');
            if (icon) {
                icon.textContent = currentTheme === 'dark' ? '☀️' : '🌙';
            }
        }

        // Update icon on load
        updateThemeIcon();

        // Header theme toggle button click handler
        if (themeToggle) {
            themeToggle.addEventListener('click', async () => {
                const currentTheme = localStorage.getItem('theme-preference') || 'auto';
                let nextTheme = currentTheme === 'light' ? 'dark' : 'light';

                // Add animation class
                themeToggle.classList.add('transitioning');

                try {
                    // Call API to save theme
                    const response = await fetch('/api/v1/user/theme', {
                        method: 'POST',
                        headers: {
                            'Content-Type': 'application/json',
                        },
                        body: JSON.stringify({ theme: nextTheme })
                    });

                    if (response.ok) {
                        // Apply theme instantly
                        window.toggleTheme(nextTheme);
                        updateThemeIcon();
                    } else {
                        console.error('Failed to save theme preference');
                        // Revert animation
                        themeToggle.classList.remove('transitioning');
                    }
                } catch (err) {
                    console.error('Error saving theme:', err);
                    themeToggle.classList.remove('transitioning');
                }
            });

            // Remove animation class after animation completes
            themeToggle.addEventListener('animationend', () => {
                themeToggle.classList.remove('transitioning');
            });
        }
