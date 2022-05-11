pub fn to_bin(mut val: i32) -> Vec<bool> {
    let mut answer = Vec::new();

    while val / 2 > 0 {
        answer.push(val % 2 == 1);
        val /= 2;
    }
    answer.push(val == 1);
    answer.reverse();
    return answer;
}


pub fn to_const_size_bin(val: i32, size: i32) -> Vec<bool> {
    let mut bin_value = to_bin(val);
    if bin_value.len() > size as usize {
        panic!("error cast num to const size bin");
    }
    let mut answer = vec![false; size as usize - bin_value.len()];
    answer.append(&mut bin_value);
    answer
}

pub fn bin_to_num(data: &[bool]) -> i32 {
    let mut result = 0;
    for i in 0..data.len() {
        if data[i] {
            result += i32::pow(2, (data.len() - 1 - i) as u32);
        }
    }

    result
}