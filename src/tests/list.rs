use super::page_numbers;

#[test]
fn pagination_links_preserve_boundaries_and_a_small_window_even_for_huge_counts() {
    for (page, pages, expected) in [
        (1, 1, vec![1]),
        (1, 2, vec![1, 2]),
        (1, 10, vec![1, 2, 3, 4, 10]),
        (5, 10, vec![1, 2, 3, 4, 5, 6, 7, 8, 10]),
        (10, 10, vec![1, 7, 8, 9, 10]),
        (1, u64::MAX, vec![1, 2, 3, 4, u64::MAX]),
        (
            u64::MAX,
            u64::MAX,
            vec![1, u64::MAX - 3, u64::MAX - 2, u64::MAX - 1, u64::MAX],
        ),
    ] {
        assert_eq!(page_numbers(page, pages).collect::<Vec<_>>(), expected);
    }
}
