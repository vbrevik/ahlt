(function() {
    const simpleRadio = document.querySelector('input[name="coa_type"][value="simple"]');
    const complexRadio = document.querySelector('input[name="coa_type"][value="complex"]');
    const complexSection = document.getElementById('complex-coa-section');

    function toggleComplexSection() {
        if (complexRadio.checked) {
            complexSection.style.display = 'block';
        } else {
            complexSection.style.display = 'none';
        }
    }

    simpleRadio.addEventListener('change', toggleComplexSection);
    complexRadio.addEventListener('change', toggleComplexSection);

    // Initialize on page load
    toggleComplexSection();

    document.getElementById('add-section-btn').addEventListener('click', function(e) {
        e.preventDefault();
        const container = document.getElementById('sections-container');

        const sectionDiv = document.createElement('div');
        sectionDiv.className = 'coa-section-item';

        const titleInput = document.createElement('input');
        titleInput.type = 'text';
        titleInput.name = 'section_titles';
        titleInput.placeholder = 'Section title';
        titleInput.className = 'form-control';

        const contentInput = document.createElement('textarea');
        contentInput.name = 'section_contents';
        contentInput.rows = 3;
        contentInput.placeholder = 'Section content...';
        contentInput.className = 'form-control';

        const removeBtn = document.createElement('button');
        removeBtn.type = 'button';
        removeBtn.className = 'btn btn-sm btn-danger remove-section-btn';
        removeBtn.textContent = 'Remove';
        removeBtn.addEventListener('click', function(e) {
            e.preventDefault();
            sectionDiv.remove();
        });

        const sectionInputDiv = document.createElement('div');
        sectionInputDiv.className = 'section-input';
        sectionInputDiv.appendChild(titleInput);
        sectionInputDiv.appendChild(contentInput);
        sectionInputDiv.appendChild(removeBtn);

        sectionDiv.appendChild(sectionInputDiv);
        container.appendChild(sectionDiv);
    });
})();
