use std::collections::{HashMap, LinkedList};

pub const MAX_SHIFT: u16 = 2 << 15 - 1;
pub const MAX_COINCIDENCE_SIZE: u16 = 258;
pub const MIN_COINCIDENCE_SIZE: u16 = 3;


pub struct Lz77ReferenceElement {
    pub offset: u16,
    pub count: u16,
}

pub enum Lz77Element {
    SimpleValue(u8),
    ReferenceValue(Lz77ReferenceElement),
}


pub fn encoding_lz77(data: &Vec<u8>) -> Vec<Lz77Element> {
    let mut answer: Vec<Lz77Element> = Vec::new();
    let mut lib: HashMap<[u8; MIN_COINCIDENCE_SIZE as usize], LinkedList<usize>> = HashMap::new();

    let mut i = 0;
    while i < data.len() {

        if i + (MIN_COINCIDENCE_SIZE as usize) - 1 < data.len() {
            let initial_i = i;
            match lib.get_mut(&data[i..i + MIN_COINCIDENCE_SIZE as usize]) {
                Some(start_index) => {
                    let mut max_slice = 0;
                    let mut index_slice = 0;

                    while start_index.len() > 10 {
                        start_index.pop_front();
                    }

                    for current_start_index_slice in start_index.iter().rev() {
                        if i - current_start_index_slice <= MAX_SHIFT as usize {
                            let mut current_slice_size = MIN_COINCIDENCE_SIZE;

                            for delta in MIN_COINCIDENCE_SIZE as usize..data.len() - i {

                                if data[i + delta] == data[current_start_index_slice + delta] &&
                                    current_slice_size < MAX_COINCIDENCE_SIZE {
                                    current_slice_size += 1;
                                } else {
                                    break;
                                }
                            }
                            if current_slice_size as u16 > max_slice {
                                max_slice = current_slice_size as u16;
                                index_slice = *current_start_index_slice as u16;
                            }
                        }
                    }
                    if max_slice != 0 {
                        answer.push(Lz77Element::ReferenceValue(
                            Lz77ReferenceElement {
                                offset: (initial_i - index_slice as usize) as u16,
                                count: max_slice,
                            }
                        ));
                        i += max_slice as usize;
                    } else {
                        answer.push(Lz77Element::SimpleValue(data[initial_i]));
                        i += 1;
                    }
                }
                None => {
                    answer.push(Lz77Element::SimpleValue(data[initial_i]));
                    i += 1;
                }
            }
            let mut key: [u8; MIN_COINCIDENCE_SIZE as usize] = Default::default();
            key.clone_from_slice(&data[initial_i..initial_i + MIN_COINCIDENCE_SIZE as usize]);
            lib.entry(key).or_insert(LinkedList::new()).push_back(initial_i);
        } else {
            answer.push(Lz77Element::SimpleValue(data[i]));
            i += 1;
        }
    }
    return answer;
}
