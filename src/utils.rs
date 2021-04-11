/// WARNING: Truncates the string to desired size if string is too long
pub fn to_fixed_length<T: Into<String>>(string: T, size: usize) -> Vec<u8> {
    let mut bytes = string.into().bytes().collect::<Vec<u8>>();

    bytes.truncate(size);

    if bytes.len() < size {
        let needed_nulls = size - bytes.len();
        let mut nulls: Vec<u8> = vec![0x00; needed_nulls];

        bytes.append(&mut nulls);
    }

    bytes
}

#[inline]
pub fn pad_to_nearest(size: usize, step: usize) -> usize {
    // Pad size to nearest step
    let rem = size % step;

    if rem == 0 {
        0
    } else {
        size + (step - rem)
    }
}

#[inline]
pub fn pad_to_nearest_with_excess(size: usize, step: usize) -> usize {
    // Pad size to nearest step
    let rem = size % step;
    size + (step - rem)
}

#[inline]
pub fn needed_to_align(size: usize, step: usize) -> usize {
    let rem = size % step;
    if rem == 0 {
        0
    } else {
        step - rem
    }
}

#[inline]
pub fn needed_to_align_with_excess(size: usize, step: usize) -> usize {
    let rem = size % step;
    step - rem
}
