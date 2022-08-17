use std::cmp::max;
use std::collections::HashMap;
use std::hash::Hash;

pub fn huffman_lengths_to_bin_code<T: Clone + Eq + Hash + Ord>(lengths: Vec<(T, i32)>) ->
                                                        (HashMap<T, Vec<bool>>, HashMap<Vec<bool>, T>) {

    let mut answer = HashMap::new();
    let mut reverse_answer = HashMap::new();

    let mut count_num = 0;
    let mut mask = 0;
    for i in 0..lengths.len() {
        if i == 0 || lengths[i - 1].1 == 0 {
            answer.insert(lengths[i].0.clone(), vec![false; max(0, lengths[i].1 as usize)]);
            reverse_answer.insert(vec![false; max(0, lengths[i].1 as usize)], lengths[i].0.clone());
            count_num = lengths[i].1 as usize;
            continue;
        } else if lengths[i].1 == lengths[i - 1].1 {
            mask += 1;
        } else if lengths[i].1 > lengths[i - 1].1 {
            mask += 1;
            while count_num != lengths[i].1 as usize {
                mask <<= 1;
                count_num += 1;
            }
        } else {
            panic!("the lengths are not sorted")
        }
        let mut bin_code: Vec<bool>;
        bin_code = Vec::new();
        let mut count = count_num as i128 - 1;
        while count >= 0 {
            bin_code.push(mask & (1 << count) != 0);
            count -= 1;
        }
        answer.insert(lengths[i].0.clone(), bin_code.clone());
        reverse_answer.insert(bin_code, lengths[i].0.clone());
    }
    (answer, reverse_answer)
}

fn merge_coins(c1: &(i32, HashMap<usize, i32>), c2: &(i32, HashMap<usize, i32>)) -> (i32, HashMap<usize, i32>){
    let w = c1.0 + c2.0;

    let mut d = c1.1.clone();
    for (k, v) in &c2.1 {
        d.entry(*k).or_insert(0);
        d.insert(*k, max(*d.get(&k).unwrap(), *v));
    }

    return (w, d);
}

pub fn bounded_huffman(max_len: i32, number_repetitions: &[i32]) -> Vec<i32>{
    if i32::pow(2, max_len as u32) < number_repetitions.len() as i32{
        panic!("huffman coding is not possible");
    }

    let mut coins = Vec::new();

    for level in (1..=max_len).rev() {
        let new_coins = number_repetitions.iter().enumerate().map(|x| {
            (*x.1, HashMap::from([(x.0, level)]))
        }).collect::<Vec<(i32, HashMap<usize, i32>)>>();

        let mut prev_coins = Vec::new();
        for i in 0..coins.len() / 2 {
            prev_coins.push(merge_coins(&coins[2 * i], &coins[2 * i + 1]));
        }

        coins.clear();
        coins = prev_coins;
        coins.extend(new_coins);
        coins.sort_by(|f, s| f.0.cmp(&s.0));
     }

    let mut res = vec![0; number_repetitions.len()];

    for i in 0..max(number_repetitions.len(), number_repetitions.len() * 2 - 2) as usize {
        for (k, v) in &coins[i].1 {
            if res[*k] < *v {
                res[*k] = *v;
            }
        }
    }

    return res;
}



