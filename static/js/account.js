    // Tab switching
    document.querySelectorAll('[role="tab"]').forEach(tab => {
        tab.addEventListener('click', () => {
            document.querySelectorAll('[role="tab"]').forEach(t => {
                t.classList.remove('active');
                t.setAttribute('aria-selected', 'false');
            });
            document.querySelectorAll('[role="tabpanel"]').forEach(p => {
                p.classList.add('hidden');
            });

            tab.classList.add('active');
            tab.setAttribute('aria-selected', 'true');
            const panelId = tab.getAttribute('aria-controls');
            document.getElementById(panelId).classList.remove('hidden');
        });
    });

    // Theme selection
    const currentTheme = localStorage.getItem('theme-preference') || 'auto';
    document.querySelectorAll('.theme-btn').forEach(btn => {
        const theme = btn.getAttribute('data-theme');
        if (theme === currentTheme) {
            btn.classList.add('active');
        }
        btn.addEventListener('click', async () => {
            document.querySelectorAll('.theme-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            window.toggleTheme(theme);
            try {
                const response = await fetch('/api/v1/user/theme', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ theme })
                });
                if (!response.ok) {
                    console.error('Failed to save theme preference to database');
                }
            } catch (err) {
                console.error('Error saving theme preference:', err);
            }
        });
    });

    // Avatar upload handling
    const avatarUpload = document.getElementById('avatar-upload');
    const avatarPreview = document.getElementById('avatar-preview');
    const avatarPlaceholder = document.getElementById('avatar-placeholder');
    const avatarActions = document.getElementById('avatar-actions');
    const avatarError = document.getElementById('avatar-error');
    const avatarSave = document.getElementById('avatar-save');
    const avatarDelete = document.getElementById('avatar-delete');
    const avatarCancel = document.getElementById('avatar-cancel');

    let selectedFile = null;
    let selectedDataUri = null;

    avatarUpload.addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;

        // Validate file type
        if (!['image/jpeg', 'image/png'].includes(file.type)) {
            avatarError.textContent = 'Only JPEG and PNG files are allowed';
            avatarError.style.display = 'block';
            return;
        }

        // Validate file size (200KB)
        if (file.size > 200 * 1024) {
            avatarError.textContent = 'File size must be less than 200KB';
            avatarError.style.display = 'block';
            return;
        }

        avatarError.style.display = 'none';
        selectedFile = file;

        // Read file as data URI
        const reader = new FileReader();
        reader.onload = (event) => {
            selectedDataUri = event.target.result;
            avatarPreview.src = selectedDataUri;
            avatarPreview.style.display = 'block';
            avatarPlaceholder.style.display = 'none';
            avatarActions.style.display = 'flex';
        };
        reader.readAsDataURL(file);
    });

    avatarSave.addEventListener('click', async () => {
        if (!selectedDataUri) return;

        const formData = new FormData();
        formData.append('csrf_token', document.querySelector('input[name="csrf_token"]').value);
        formData.append('action', 'upload_avatar');
        formData.append('avatar_data_uri', selectedDataUri);

        try {
            const response = await fetch('/account/profile', {
                method: 'POST',
                body: formData
            });

            if (response.ok) {
                // Reload page to show updated avatar
                window.location.reload();
            } else {
                avatarError.textContent = 'Failed to save avatar';
                avatarError.style.display = 'block';
            }
        } catch (err) {
            avatarError.textContent = 'Error uploading avatar';
            avatarError.style.display = 'block';
        }
    });

    avatarDelete.addEventListener('click', async () => {
        if (!confirm('Delete your avatar?')) return;

        const formData = new FormData();
        formData.append('csrf_token', document.querySelector('input[name="csrf_token"]').value);
        formData.append('action', 'delete_avatar');

        try {
            const response = await fetch('/account/profile', {
                method: 'POST',
                body: formData
            });

            if (response.ok) {
                window.location.reload();
            } else {
                avatarError.textContent = 'Failed to delete avatar';
                avatarError.style.display = 'block';
            }
        } catch (err) {
            avatarError.textContent = 'Error deleting avatar';
            avatarError.style.display = 'block';
        }
    });

    avatarCancel.addEventListener('click', () => {
        avatarUpload.value = '';
        selectedFile = null;
        selectedDataUri = null;
        avatarPreview.style.display = 'none';
        avatarPlaceholder.style.display = 'block';
        avatarActions.style.display = 'none';
        avatarError.style.display = 'none';
    });
