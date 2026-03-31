#[derive(Debug, Clone, serde::Deserialize)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}
