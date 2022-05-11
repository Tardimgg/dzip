extern crate core;

use std::cmp::min;
use std::cmp::Ordering::{Equal};
use std::collections::HashMap;
use std::fmt::{Debug};
use std::fs;
use std::fs::{File};

use std::io::{BufWriter, Read, Write};

use crate::bin_num::{bin_to_num, to_const_size_bin};
use crate::compared_element::ComparedElement;
use crate::DeflateElementType::{EndBlock, JustElement, LengthMatch, LengthMatchWithAdd,
                                LengthMatchWithBinAdd, LengthMatchWithFifthAdd, LengthMatchWithFourthAdd,
                                LengthMatchWithThirdAdd, MaxMatchLength};
use crate::huffman::{bounded_huffman, huffman_lengths_to_bin_code};
use crate::lz77::{encoding_lz77, Lz77Element};
use crate::lz77::Lz77Element::{ReferenceValue, SimpleValue};
use crate::lz77::MAX_COINCIDENCE_SIZE;

mod lz77;
mod huffman;
mod compared_element;
mod bin_num;

const SEQUENCE_LENGTH_COMMAND: [i32; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];


#[derive(Debug, Clone)]
struct DeflateOffset {
    main_value: u8,
    additional_bits: u16,
}

impl DeflateOffset {
    fn new(offset: u16) -> DeflateOffset {
        if offset <= 4 {
            return DeflateOffset {
                main_value: offset as u8 - 1,
                additional_bits: 0,
            };
        }
        let base_degree = ((offset - 1) as f32).log2() as u8;
        let base_num = u16::pow(2, base_degree as u32) + 1;

        let delta;
        let mid = base_num + (base_num >> 1) - 1;

        let main_value = if offset <= mid {
            delta = offset - base_num;
            base_degree << 1
        } else {
            delta = offset - mid - 1;
            (base_degree << 1) + 1
        };

        DeflateOffset {
            main_value,
            additional_bits: delta,
        }
    }

    fn get_base_offset(&self) -> u16 {
        if self.main_value < 4 {
            return (self.main_value + 1) as u16;
        }

        let base_num = u16::pow(2, (self.main_value >> 1) as u32) + 1;
        if (self.main_value & 1) == 0 {
            return base_num;
        }

        base_num + (base_num >> 1)
    }

    fn get_number_additional_bits(&self) -> u8 {
        if self.main_value <= 3 {
            return 0;
        }

        (self.main_value >> 1) - 1
    }
}


#[derive(Clone)]
enum DeflateLength {
    SimpleLength(u8),
    RetryPrevious(u8),
    RetryZero(u8),
    BigRetryZero(u8),
}

#[derive(Clone)]
enum DeflateElementType {
    JustElement(u8),
    EndBlock,
    LengthMatch(u16),
    LengthMatchWithAdd(u16, u8),
    LengthMatchWithBinAdd(u16, u8),
    LengthMatchWithThirdAdd(u16, u8),
    LengthMatchWithFourthAdd(u16, u8),
    LengthMatchWithFifthAdd(u16, u8),
    MaxMatchLength,
}

impl DeflateElementType {
    fn get_number_additional_bits(&self) -> u8 {
        match self {
            JustElement(_) => { 0 }
            EndBlock => { 0 }
            LengthMatch(_) => { 0 }
            LengthMatchWithAdd(_, _) => { 1 }
            LengthMatchWithBinAdd(_, _) => { 2 }
            LengthMatchWithThirdAdd(_, _) => { 3 }
            LengthMatchWithFourthAdd(_, _) => { 4 }
            LengthMatchWithFifthAdd(_, _) => { 5 }
            MaxMatchLength => { 0 }
        }
    }

    fn get_additional_bits(&self) -> u8 {
        match self {
            JustElement(_) => panic!("JustElement not have additional bits"),
            EndBlock => panic!("EndBlock not have additional bits"),
            LengthMatch(_) => panic!("LengthMatch not have additional bits"),
            LengthMatchWithAdd(_, add) => *add,
            LengthMatchWithBinAdd(_, add) => *add,
            LengthMatchWithThirdAdd(_, add) => *add,
            LengthMatchWithFourthAdd(_, add) => *add,
            LengthMatchWithFifthAdd(_, add) => *add,
            MaxMatchLength => panic!("MaxMatchLength not have additional bits")
        }
    }
}

fn deflate_len_to_compared(elem: DeflateLength) -> ComparedElement<DeflateLength> {
    match elem {
        DeflateLength::SimpleLength(v) => { ComparedElement::new(elem, v as i32) }
        DeflateLength::RetryPrevious(_) => { ComparedElement::new(elem, 16) }
        DeflateLength::RetryZero(_) => { ComparedElement::new(elem, 17) }
        DeflateLength::BigRetryZero(_) => { ComparedElement::new(elem, 18) }
    }
}

fn lz77_elem_to_compared_deflate_elem_type(elem: &Lz77Element) -> ComparedElement<DeflateElementType> {
    match elem {
        SimpleValue(v) => {
            ComparedElement::new(JustElement(*v), *v as i32)
        }
        ReferenceValue(v) => {
            match v.count {
                c @ 3..=10 => ComparedElement::new(LengthMatch(c + 254), (c + 254) as i32),
                c @ 11..=18 => {
                    ComparedElement::new(LengthMatchWithAdd((c - 1) / 2 + 260, ((c - 1) & 1) as u8),
                                         ((c - 1) / 2 + 260) as i32)
                }
                c @ 19..=34 => {
                    ComparedElement::new(LengthMatchWithBinAdd((c + 1) / 4 + 264, ((c + 1) % 4) as u8),
                                         ((c + 1) / 4 + 264) as i32)
                }
                c @ 35..=66 => {
                    ComparedElement::new(LengthMatchWithThirdAdd((c + 5) / 8 + 268, ((c + 5) % 8) as u8),
                                         ((c + 5) / 8 + 268) as i32)
                }
                c @ 67..=130 => {
                    ComparedElement::new(LengthMatchWithFourthAdd((c - 3) / 16 + 273, ((c - 3) % 16) as u8),
                                         ((c - 3) / 16 + 273) as i32)
                }
                c @ 131..=257 => {
                    ComparedElement::new(LengthMatchWithFifthAdd((c - 3) / 32 + 277, ((c - 3) % 32) as u8),
                                         ((c - 3) / 32 + 277) as i32)
                }
                258 => ComparedElement::new(MaxMatchLength, 285),
                _ => panic!("length reference value > 258 or < 3")
            }
        }
    }
}

fn to_compared_deflate_element_type(id: i32, add: u8) -> ComparedElement<DeflateElementType> {
    match id {
        0..=255 => ComparedElement::new(JustElement(id as u8), id),
        256 => ComparedElement::new(EndBlock, id as i32),
        257..=264 => ComparedElement::new(LengthMatch(id as u16), id),
        265..=268 => ComparedElement::new(LengthMatchWithAdd(id as u16, add), id),
        269..=272 => ComparedElement::new(LengthMatchWithBinAdd(id as u16, add), id),
        273..=276 => ComparedElement::new(LengthMatchWithThirdAdd(id as u16, add), id),
        277..=280 => ComparedElement::new(LengthMatchWithFourthAdd(id as u16, add), id),
        281..=284 => ComparedElement::new(LengthMatchWithFifthAdd(id as u16, add), id),
        285 => ComparedElement::new(MaxMatchLength, id as i32),
        _ => panic!("encoded code contains mistake")
    }
}

fn get_deflate_copy_length(elem: &DeflateElementType) -> u16 {
    match elem {
        JustElement(_) => panic!("JustElement not have copy length"),
        EndBlock => panic!("EndBlock not have copy length"),
        LengthMatch(l) => { l - 254 }
        LengthMatchWithAdd(l, add) => { ((l - 265) << 1) + 11 + *add as u16 }
        LengthMatchWithBinAdd(l, add) => { ((l - 269) << 2) + 19 + *add as u16 }
        LengthMatchWithThirdAdd(l, add) => { ((l - 273) << 3) + 35 + *add as u16 }
        LengthMatchWithFourthAdd(l, add) => { ((l - 277) << 4) + 67 + *add as u16 }
        LengthMatchWithFifthAdd(l, add) => { ((l - 281) << 5) + 131 + *add as u16 }
        MaxMatchLength => { MAX_COINCIDENCE_SIZE }
    }
}


fn encoding_sequence_length(sequence_of_length: &[i32]) -> Vec<DeflateLength> {
    let mut answer = Vec::new();

    let mut number_of_zeros = 0;

    let mut i = 0;
    while i < sequence_of_length.len() {
        if sequence_of_length[i] == 0 {
            number_of_zeros += 1;
        }
        if sequence_of_length[i] != 0 {
            while number_of_zeros >= 3 {
                if number_of_zeros > 11 {
                    answer.push(DeflateLength::BigRetryZero(min(138, number_of_zeros)));
                    number_of_zeros -= min(138, number_of_zeros);
                } else {
                    answer.push(DeflateLength::RetryZero(min(10, number_of_zeros)));
                    number_of_zeros -= min(10, number_of_zeros);
                }
            }
            while number_of_zeros != 0 {
                answer.push(DeflateLength::SimpleLength(0));
                number_of_zeros -= 1;
            }

            if i > 0 && sequence_of_length[i] == sequence_of_length[i - 1] {
                let mut count = 1;
                while i + 1 < sequence_of_length.len() &&
                    count < 6 && sequence_of_length[i] == sequence_of_length[i + 1] {
                    count += 1;
                    i += 1;
                }
                if count >= 3 {
                    answer.push(DeflateLength::RetryPrevious(count));
                } else {
                    for _ in 0..count {
                        answer.push(DeflateLength::SimpleLength(sequence_of_length[i] as u8));
                    }
                }
            } else {
                answer.push(DeflateLength::SimpleLength(sequence_of_length[i] as u8));
            }
        }
        i += 1;
    }

    answer
}

fn bin_write_on_file(value: Vec<bool>, path: &str) {
    let mut writer = BufWriter::new(File::create(path).unwrap());

    let mut start = 0;
    while start < value.len() {
        let mut element: [u8; 1] = [0];

        element[0] = value.iter().skip(start).take(8).fold(0, |result, &bit| {
            (result << 1) ^ if bit { 1 } else { 0 }
        });

        writer.write(&element).expect("Write operation error");
        start += 8;
    }
}

fn sort_huffman_lengths<T>(data: &mut Vec<(ComparedElement<T>, i32)>) {
    data.sort_by(|f, s| {
        let mut res = f.1.cmp(&s.1);
        if res == Equal {
            res = f.0.sorting_value.cmp(&s.0.sorting_value);
        }
        res
    });
}


fn huffman_encoding<T: Clone>(max_depth: i32, lang_size: usize, data: &[ComparedElement<T>]) ->
(HashMap<ComparedElement<T>, Vec<bool>>,
 Vec<i32>) {
    let mut lang = Vec::new();
    let mut repetition_counter = vec![0; lang_size];
    for val in data {
        repetition_counter[val.sorting_value as usize] += 1;

        if repetition_counter[val.sorting_value as usize] == 1 {
            lang.push(val);
        }
    }

    let required_data = repetition_counter
        .iter()
        .filter_map(|x| if *x == 0 { None } else { Some(*x) })
        .collect::<Vec<i32>>();

    let required_lengths = bounded_huffman(max_depth, required_data.as_slice());

    let mut i = 0;
    let all_lengths = repetition_counter
        .iter()
        .map(|x| {
            if *x == 0 {
                0
            } else {
                i += 1;
                required_lengths[i - 1]
            }
        })
        .collect::<Vec<i32>>();

    let mut repetition_element = Vec::new();
    for val in lang {
        let sv = val.sorting_value;
        repetition_element.push((val.clone(), all_lengths[sv as usize]))
    }

    sort_huffman_lengths(&mut repetition_element);
    let bin_codes = huffman_lengths_to_bin_code(repetition_element).0;

    (bin_codes, all_lengths)
}

fn deflate_block_decoding(answer: &mut Vec<u8>, data: &[bool]) -> (usize, bool) {

    let hlit = bin_to_num(&data[3..8]);
    let hdist = bin_to_num(&data[8..13]);
    let hclen = bin_to_num(&data[13..17]) as usize;

    if data[1] || !data[2] {
        panic!("not supported");
    }

    let mut deflate_len_elements = Vec::new();

    let mut read_index = 17;
    for sequence_index in 0..hclen + 4 {
        let index = SEQUENCE_LENGTH_COMMAND[sequence_index] as usize;
        let num = bin_to_num(&data[read_index..read_index + 3]);

        if num != 0 {
            deflate_len_elements.push((match index {
                0..=15 => ComparedElement::new(
                    DeflateLength::SimpleLength(index as u8),
                    index as i32),

                16 => ComparedElement::new(DeflateLength::RetryPrevious(0), 16),
                17 => ComparedElement::new(DeflateLength::RetryZero(0), 17),
                18 => ComparedElement::new(DeflateLength::BigRetryZero(0), 18),
                _ => panic!("encoded code contains mistake")
            }, num));
        }
        read_index += 3;
    }

    sort_huffman_lengths(&mut deflate_len_elements);
    let lang_len_elements = huffman_lengths_to_bin_code(deflate_len_elements).1;

    let mut deflate_elements = Vec::new();

    let mut temp = Vec::new();
    let mut i: usize = 0;
    while i < (hlit + 257) as usize {
        temp.push(data[read_index]);
        read_index += 1;
        if let Some(elem) = lang_len_elements.get(&temp) {
            match elem.value {
                DeflateLength::SimpleLength(v) => {
                    if v != 0 {
                        deflate_elements.push((to_compared_deflate_element_type(i as i32, 0), v as i32));
                    }
                    i += 1;
                }
                DeflateLength::RetryPrevious(_) => {
                    let count = bin_to_num(&data[read_index..read_index + 2]) + 3;
                    let length_previous_code = deflate_elements.last().unwrap().1;
                    for _ in 0..count {
                        deflate_elements.push((to_compared_deflate_element_type(i as i32, 0),
                                               length_previous_code));
                        i += 1;
                    }
                    read_index += 2;
                }
                DeflateLength::RetryZero(_) => {
                    let count = bin_to_num(&data[read_index..read_index + 3]) + 3;
                    i += count as usize;
                    read_index += 3;
                }
                DeflateLength::BigRetryZero(_) => {
                    let count = bin_to_num(&data[read_index..read_index + 7]) + 11;
                    i += count as usize;
                    read_index += 7;
                }
            }
            temp.clear();
        }
    }

    sort_huffman_lengths(&mut deflate_elements);
    let lang_deflate_elements = huffman_lengths_to_bin_code(deflate_elements).1;

    let mut deflate_offset_elements = Vec::new();

    let mut temp = Vec::new();
    let mut i: usize = 0;
    while i < hdist as usize {
        temp.push(data[read_index]);
        read_index += 1;
        if let Some(elem) = lang_len_elements.get(&temp) {
            match elem.value {
                DeflateLength::SimpleLength(v) => {
                    if v != 0 {
                        deflate_offset_elements.push((ComparedElement::new(DeflateOffset {
                            main_value: i as u8,
                            additional_bits: 0,
                        }, i as i32), v as i32));
                    }
                    i += 1;
                }
                DeflateLength::RetryPrevious(_) => {
                    let count = bin_to_num(&data[read_index..read_index + 2]) + 3;
                    let length_previous_code = deflate_offset_elements.last().unwrap().1;

                    for _ in 0..count {
                        deflate_offset_elements.push((ComparedElement::new(DeflateOffset {
                            main_value: i as u8,
                            additional_bits: 0,
                        }, i as i32), length_previous_code));

                        i += 1;
                    }
                    read_index += 2;
                }
                DeflateLength::RetryZero(_) => {
                    let count = bin_to_num(&data[read_index..read_index + 3]) + 3;
                    i += count as usize;
                    read_index += 3;
                }
                DeflateLength::BigRetryZero(_) => {
                    let count = bin_to_num(&data[read_index..read_index + 7]) + 11;
                    i += count as usize;
                    read_index += 7;
                }
            }
            temp.clear();
        }
    }

    sort_huffman_lengths(&mut deflate_offset_elements);
    let lang_deflate_offset_elements = huffman_lengths_to_bin_code(deflate_offset_elements).1;

    let mut length_match = 0;
    let mut finding_offset = false;
    let mut temp = Vec::new();
    loop {
        temp.push(data[read_index]);
        read_index += 1;

        if finding_offset {
            if let Some(elem) = lang_deflate_offset_elements.get(&temp) {
                let base_num = elem.value.get_base_offset();

                let number_additional_bits = elem.value.get_number_additional_bits();

                let offset = base_num + bin_to_num(&data[read_index..read_index + number_additional_bits as usize]) as u16;
                read_index += number_additional_bits as usize;

                for _ in 0..length_match {
                    answer.push(answer[answer.len() - offset as usize]);
                }
                temp.clear();
                finding_offset = false;
            }
        } else if let Some(elem) = lang_deflate_elements.get(&temp) {
            match elem.value {
                JustElement(v) => answer.push(v),
                EndBlock => break,
                _ => {
                    let base_length_match = get_deflate_copy_length(&elem.value) as usize;

                    let number_additional_bits = elem.value.get_number_additional_bits();

                    length_match = base_length_match + bin_to_num(&data[read_index..read_index + number_additional_bits as usize]) as usize;
                    read_index += number_additional_bits as usize;

                    finding_offset = true;
                }
            }
            temp.clear();
        }
    }

    (read_index, data[0] == true)
}


fn deflate_decoding(data: Vec<u8>) -> Vec<u8> {
    let mut bin_data = Vec::new();

    let mut offset: i32 = -1;
    for i in 0..data.len() {
        if i + 1 < data.len() {
            for chr in format!("{:8b}", data[i]).chars() {
                bin_data.push(chr == '1');
            }
        } else {
            let bin_value = format!("{:b}", data[i]);
            for _ in 0..offset as usize - bin_value.len() {
                bin_data.push(false);
            }
            for chr in bin_value.chars() {
                bin_data.push(chr == '1');
            }
        }
        if offset == -1 && bin_data.len() >= 3 {
            offset = bin_to_num(&bin_data[0..3]);
            if offset == 0 {
                offset = 8;
            }
        }
    }
    let mut start_block = 3;
    let mut answer: Vec<u8> = Vec::new();

    loop {
        let local_data = &bin_data[start_block..];
        let (shift, is_end_block) = deflate_block_decoding(&mut answer, local_data);
        start_block += shift;

        if is_end_block {
            if start_block != bin_data.len() {
                panic!("Internal error");
            }
            break;
        }
    }
    answer
}

fn deflate_block_encoding(lz77_data: &[Lz77Element], is_end_block: bool) -> Vec<bool> {
    let mut bin_result = Vec::new();

    let mut deflate_elements = lz77_data
        .iter()
        .map(|x| lz77_elem_to_compared_deflate_elem_type(x))
        .collect::<Vec<ComparedElement<DeflateElementType>>>();
    deflate_elements.push(ComparedElement::new(EndBlock, 256));

    let (bin_deflate_codes, all_deflate_lengths) = huffman_encoding(15,
                                                                    286,
                                                                    deflate_elements.as_slice());

    let offset_elements = lz77_data
        .iter()
        .filter_map(|x| match x {
            SimpleValue(_) => { None }
            ReferenceValue(v) => {
                let offset = DeflateOffset::new(v.offset);
                let compared_value = offset.main_value;
                Some(ComparedElement::new(offset, compared_value as i32))
            }
        }).collect::<Vec<ComparedElement<DeflateOffset>>>();

    let (bin_offset_codes, all_offset_lengths) = huffman_encoding(15,
                                                                      30,
                                                                      offset_elements.as_slice());


    let mut encoded_sequence_lengths = encoding_sequence_length(
        all_deflate_lengths.as_slice());

    let encoded_sequence_offset_lengths = encoding_sequence_length(
        all_offset_lengths.as_slice());

    encoded_sequence_lengths.extend(encoded_sequence_offset_lengths);

    let deflate_length_elements = encoded_sequence_lengths
        .iter()
        .map(|x| deflate_len_to_compared(x.clone()))
        .collect::<Vec<ComparedElement<DeflateLength>>>();

    let (bin_deflate_len_codes, all_deflate_len_lengths) = huffman_encoding(7,
                                                                            19,
                                                                            deflate_length_elements.as_slice());


    let hlit = bin_deflate_codes.keys().filter_map(|x|
        match &x.value {
            JustElement(_) => { None }
            EndBlock => { None }
            _ => { Some(x.sorting_value) }
        })
        .max().unwrap_or(256) - 256;

    let hdist = bin_offset_codes.keys().map(|v| v.sorting_value).max().unwrap_or(-1) + 1;

    let mut hclen = SEQUENCE_LENGTH_COMMAND.len() - 4;
    for v in SEQUENCE_LENGTH_COMMAND.iter().rev() {
        if all_deflate_len_lengths[*v as usize] == 0 {
            hclen -= 1;
        } else {
            break;
        }
    }

    bin_result.append(&mut vec![is_end_block, false, true]);

    bin_result.append(&mut to_const_size_bin(hlit, 5));
    bin_result.append(&mut to_const_size_bin(hdist, 5));
    bin_result.append(&mut to_const_size_bin(hclen as i32, 4));

    for length_index in &SEQUENCE_LENGTH_COMMAND[..hclen + 4] {
        let mut bin_value = to_const_size_bin(all_deflate_len_lengths[*length_index as usize], 3);
        bin_result.append(&mut bin_value)
    }

    encoded_sequence_lengths.into_iter().for_each(|v| {
        bin_result.extend(bin_deflate_len_codes.get(&deflate_len_to_compared(v.clone())).unwrap());

        match v {
            DeflateLength::RetryPrevious(v) => {
                bin_result.extend(to_const_size_bin(v as i32 - 3, 2))
            }
            DeflateLength::RetryZero(v) => {
                bin_result.extend(to_const_size_bin(v as i32 - 3, 3))
            }
            DeflateLength::BigRetryZero(v) => {
                bin_result.extend(to_const_size_bin(v as i32 - 11, 7))
            }
            _ => ()
        };
    });

    for value in lz77_data {
        let compared_deflate_elem = lz77_elem_to_compared_deflate_elem_type(&value);
        bin_result.extend(bin_deflate_codes.get(&compared_deflate_elem).unwrap());

        if let ReferenceValue(v) = value {
            let number_additional_bits_for_match = compared_deflate_elem.value.get_number_additional_bits() as i32;

            if number_additional_bits_for_match != 0 {
                bin_result.extend(to_const_size_bin(compared_deflate_elem.value.get_additional_bits() as i32,
                                                    number_additional_bits_for_match));
            }

            let offset = DeflateOffset::new(v.offset);
            let compared_value = offset.main_value;
            let additional_bits = offset.additional_bits as i32;
            let number_additional_bits_for_offset = offset.get_number_additional_bits() as i32;

            bin_result.extend(bin_offset_codes.get(&ComparedElement::new(offset, compared_value as i32)).unwrap());

            if number_additional_bits_for_offset != 0 {
                bin_result.extend(to_const_size_bin(additional_bits, number_additional_bits_for_offset));
            }
        }
    }
    bin_result.extend(bin_deflate_codes.get(&ComparedElement::new(EndBlock, 256)).unwrap());

    bin_result
}

fn deflate_encoding(data: Vec<u8>) -> Vec<bool> {
    let lz77_result = encoding_lz77(&data);

    let mut bin_result: Vec<bool> = vec![false; 3];

    let step = (i32::pow(2, 16) - 1) as usize;
    for start_block in (0..lz77_result.len()).step_by(step) {
        let lz77_local;
        let mut is_end_block = false;

        if lz77_result.len() < start_block + step {
            lz77_local = &lz77_result[start_block..lz77_result.len()];
            is_end_block = true;
        } else {
            lz77_local = &lz77_result[start_block..start_block + step];
        }

        bin_result.extend(deflate_block_encoding(lz77_local, is_end_block));
    }

    let bin_size = to_const_size_bin((bin_result.len() % 8) as i32, 3);
    for i in 0..3 {
        bin_result[i] = bin_size[i];
    }

    bin_result
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<String>>();

    if args.len() > 2 || args.len() == 0 {
        println!("Incorrect data");
    } else {
        if args.len() == 1 {
            let mut file = File::open(args[0].as_str()).expect("no file found");
            let metadata = fs::metadata(args[0].as_str()).expect("unable to read metadata");
            let mut data = Vec::with_capacity(metadata.len() as usize);
            file.read_to_end(&mut data).expect("buffer overflow");

            bin_write_on_file(deflate_encoding(data), format!("{}.dzip", args[0]).as_str());
            println!("Successful");
        } else if args[0] == "-d" {
            let mut file = File::open(args[1].as_str()).expect("no file found");
            let metadata = fs::metadata(args[1].as_str()).expect("unable to read metadata");
            let mut data = Vec::with_capacity(metadata.len() as usize);
            file.read_to_end(&mut data).expect("buffer overflow");

            let result = deflate_decoding(data);

            let mut writer = BufWriter::new(File::create(args[1].replace(".dzip", "(1)")).unwrap());

            writer.write(result.as_slice()).expect("Write operation error");
            writer.flush().expect("Flush operation error");
        } else {
            println!("Incorrect data");
        }
    }
}
