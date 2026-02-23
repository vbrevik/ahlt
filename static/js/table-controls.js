function changePerPage(n) {
    const url = new URL(window.location.href);
    url.searchParams.set('per_page', n);
    url.searchParams.set('page', '1');
    window.location.href = url.toString();
}
