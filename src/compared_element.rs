use std::cmp::Ordering;
use std::cmp::Ordering::Equal;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct ComparedElement<T> {
    pub value: T,
    pub sorting_value: i32
}

impl<T> ComparedElement<T> {

    pub fn new(value: T, sorting_value: i32) -> ComparedElement<T>{
        ComparedElement{
            value,
            sorting_value
        }
    }
}

impl<T> Hash for ComparedElement<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sorting_value.hash(state);
    }
}

impl<T> Ord for ComparedElement<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sorting_value.cmp(&other.sorting_value)
    }
}

impl<T> PartialOrd for ComparedElement<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> PartialEq for ComparedElement<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Equal
    }
}

impl<T> Eq for ComparedElement<T> {}