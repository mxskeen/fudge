use crate::item::ResultItem;
use crate::query::Query;

pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    /// Keywords (lower-case, e.g. "kill", "/yt") that route a query exclusively to this provider.
    fn keywords(&self) -> &[&'static str] {
        &[]
    }
    fn query(&self, q: &Query) -> Vec<ResultItem>;
    /// Re-scan whatever the provider caches (apps, files). Must not block long.
    fn refresh(&self) {}
}
